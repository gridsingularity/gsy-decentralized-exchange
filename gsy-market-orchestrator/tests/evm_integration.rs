use ethers::{
    prelude::*,
    solc::{Project, ProjectPathsConfig},
    utils::Anvil,
};
use gsy_market_orchestrator::{
    chain_connector::{GsyMarketOrchestratorNodeClient, MarketChainClient, NewMarket},
    config::{Config, OffchainStorageTransport},
};
use primitives::{
    utils::{generate_market_id, parse_uuid_or_hex_bytes16},
    MarketType, MatchingAlgorithm,
};
use std::{fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

abigen!(
    MockMarketControllerReader,
    r#"[
        function markets(bytes16 marketId) external view returns (bytes16, bytes16, uint64, uint64, uint64, uint64, uint8, uint8)
    ]"#
);

#[tokio::test]
async fn test_evm_market_controller_client_creates_markets() {
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

            struct NewMarket {
                bytes16 marketId;
                bytes16 communityId;
                uint64 openingTime;
                uint64 closingTime;
                uint64 deliveryStartTime;
                uint64 deliveryEndTime;
                uint8 marketType;
                uint8 matchingAlgorithm;
            }

            mapping(bytes16 => NewMarket) public markets;

            constructor() {
                roles[msg.sender][ORCHESTRATOR_ROLE] = true;
            }

            function hasRole(bytes32 role, address account) external view returns (bool) {
                return roles[account][role];
            }

            function marketsExist(bytes16[] calldata marketIds) external view returns (bool[] memory exists) {
                exists = new bool[](marketIds.length);
                for (uint256 index = 0; index < marketIds.length; index++) {
                    exists[index] = markets[marketIds[index]].marketId != bytes16(0);
                }
            }

            function createMarkets(NewMarket[] calldata newMarkets) external {
                require(roles[msg.sender][ORCHESTRATOR_ROLE], "missing orchestrator role");
                for (uint256 index = 0; index < newMarkets.length; index++) {
                    if (markets[newMarkets[index].marketId].marketId != bytes16(0)) {
                        continue;
                    }
                    markets[newMarkets[index].marketId] = newMarkets[index];
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
        market_creation_batch_size: 50,
    };

    let orchestrator_client = GsyMarketOrchestratorNodeClient::new(&config).await.unwrap();

    assert!(orchestrator_client.is_operator_registered().await.unwrap());

    let community_id = "11111111-1111-4111-8111-111111111111";
    let delivery_start = 1_700_000_100;
    let new_markets = [MarketType::Spot, MarketType::Flex]
        .into_iter()
        .map(|market_type| NewMarket {
            market_id: generate_market_id(community_id, market_type.clone(), delivery_start),
            community_id: parse_uuid_or_hex_bytes16(community_id).unwrap(),
            opening_time: delivery_start - 1800,
            closing_time: delivery_start + 1800,
            delivery_start_time: delivery_start,
            delivery_end_time: delivery_start + 900,
            market_type: market_type.to_evm(),
            matching_algorithm: MatchingAlgorithm::PayAsClear.to_evm(),
        })
        .collect::<Vec<_>>();
    let market_ids = new_markets
        .iter()
        .map(|market| market.market_id)
        .collect::<Vec<_>>();
    let unknown_market_id = [0x42; 16];
    assert_eq!(
        orchestrator_client
            .markets_exist(market_ids.clone())
            .await
            .unwrap(),
        vec![false, false]
    );

    orchestrator_client
        .create_markets(new_markets.clone())
        .await
        .unwrap();

    // One call answers for every ID, in request order.
    assert_eq!(
        orchestrator_client
            .markets_exist(vec![market_ids[1], unknown_market_id, market_ids[0]])
            .await
            .unwrap(),
        vec![true, false, true]
    );

    let reader = MockMarketControllerReader::new(contract_address, client.clone());
    for market in &new_markets {
        let stored = reader.markets(market.market_id).call().await.unwrap();
        assert_eq!(
            stored,
            (
                market.market_id,
                market.community_id,
                market.opening_time,
                market.closing_time,
                market.delivery_start_time,
                market.delivery_end_time,
                market.market_type,
                market.matching_algorithm,
            )
        );
    }

    // Resending succeeds and leaves the stored record unchanged.
    let resent = NewMarket {
        closing_time: delivery_start + 60,
        ..new_markets[0].clone()
    };
    orchestrator_client
        .create_markets(vec![resent])
        .await
        .unwrap();
    let stored = reader
        .markets(new_markets[0].market_id)
        .call()
        .await
        .unwrap();
    assert_eq!(stored.3, new_markets[0].closing_time);
}
