use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_market_orchestrator::{
    chain_connector::{GsyMarketOrchestratorNodeClient, MarketChainClient},
    config::{Config, OffchainStorageTransport},
    orchestrator::generate_market_id,
};
use primitives::MarketType;
use std::{fs::File, io::Write, sync::Arc, time::Duration};
use tempfile::TempDir;

#[tokio::test]
async fn test_evm_market_controller_client_updates_status_batch() {
    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await.unwrap();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir).unwrap();
    let source_path = contracts_dir.join("MockMarketController.sol");

    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.20;
        contract MockMarketController {
            bytes32 public constant ORCHESTRATOR_ROLE = keccak256("ORCHESTRATOR_ROLE");
            mapping(address => mapping(bytes32 => bool)) private roles;
            mapping(bytes16 => bool) public marketStatus;

            event MarketStatusUpdated(bytes16 indexed marketId, bool isOpen);

            constructor() {
                roles[msg.sender][ORCHESTRATOR_ROLE] = true;
            }

            function hasRole(bytes32 role, address account) external view returns (bool) {
                return roles[account][role];
            }

            function isMarketOpen(bytes16 marketId) external view returns (bool) {
                return marketStatus[marketId];
            }

            function setMarketStatuses(bytes16[] calldata marketIds, bool isOpen) external {
                require(roles[msg.sender][ORCHESTRATOR_ROLE], "missing orchestrator role");
                for (uint256 index = 0; index < marketIds.length; index++) {
                    marketStatus[marketIds[index]] = isOpen;
                    emit MarketStatusUpdated(marketIds[index], isOpen);
                }
            }
        }
    "#;
    {
        let mut file = File::create(&source_path).unwrap();
        file.write_all(source.as_bytes()).unwrap();
    }

    let paths = ProjectPathsConfig::builder()
        .root(temp_dir.path())
        .sources(contracts_dir)
        .build()
        .unwrap();
    let project = Project::builder()
        .paths(paths)
        .ephemeral()
        .no_artifacts()
        .build()
        .unwrap();

    let compiled = project.compile().unwrap();
    let output = compiled.output();

    for err in &output.errors {
        if err.severity == ethers::solc::artifacts::Severity::Error {
            panic!("Solidity compilation error: {}", err.message);
        }
    }

    let contract_list = output
        .contracts
        .values()
        .flat_map(|inner| inner.iter())
        .find(|(name, _)| *name == "MockMarketController")
        .map(|(_, artifact)| artifact)
        .expect("MockMarketController artifact not found");

    let contract = &contract_list
        .first()
        .expect("No versioned contract found in artifact")
        .contract;

    let bytecode = contract
        .evm
        .as_ref()
        .expect("No EVM object found")
        .bytecode
        .as_ref()
        .expect("No bytecode found")
        .object
        .as_bytes()
        .expect("Bytecode not bytes")
        .clone();

    let abi = contract.abi.as_ref().expect("No ABI found").clone();
    let factory = ContractFactory::new(abi.into(), bytecode, client.clone());
    let contract = factory.deploy(()).unwrap().send().await.unwrap();
    let contract_address = contract.address();

    let config = Config {
        evm_node_url: ws_endpoint.clone(),
        market_controller_address: contract_address,
        orchestrator_signer_private_key:
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string(),
        tick_interval_seconds: 1,
        look_ahead_hours: 1,
        offchain_storage_transport: OffchainStorageTransport::Http,
        offchain_storage_url: "http://localhost:8080".to_string(),
    };

    let orchestrator_client = GsyMarketOrchestratorNodeClient::new(&config).await.unwrap();

    assert!(orchestrator_client.is_operator_registered().await.unwrap());

    let market_ids = vec![
        generate_market_id(MarketType::Spot, 1_700_000_000),
        generate_market_id(MarketType::Flex, 1_700_000_000),
    ];
    for market_id in &market_ids {
        assert!(!orchestrator_client
            .get_market_status(*market_id)
            .await
            .unwrap());
    }

    orchestrator_client
        .update_market_statuses(market_ids.clone(), true)
        .await
        .unwrap();

    let mut markets_open = false;
    for _ in 0..20 {
        let first_open = orchestrator_client
            .get_market_status(market_ids[0])
            .await
            .unwrap();
        let second_open = orchestrator_client
            .get_market_status(market_ids[1])
            .await
            .unwrap();
        if first_open && second_open {
            markets_open = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(
        markets_open,
        "Markets were not opened on-chain after setMarketStatuses transaction"
    );
}
