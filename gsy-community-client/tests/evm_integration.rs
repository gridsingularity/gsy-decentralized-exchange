use ethers::{prelude::*, utils::Anvil};
use ethers_solc::{artifacts::Severity, Project, ProjectPathsConfig};
use gsy_community_client::node_connector::orders::publish_orders;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::MarketType;
use std::{fs::File, io::Write, sync::Arc};
use tempfile::TempDir;

abigen!(
    MockOrderRegistry,
    r#"[
        function placedCount() external view returns (uint256)
        function lastOwner() external view returns (address)
        function lastNonce() external view returns (uint64)
        function lastIsBid() external view returns (bool)
    ]"#
);

const TEST_PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

fn test_market() -> MarketTopologySchema {
    MarketTopologySchema {
        market_id: format!("0x{}", "11".repeat(32)),
        market_type: MarketType::Spot,
        community_uuid: "community-1".to_string(),
        community_name: "Community".to_string(),
        time_slot: 1_700_000_000u32,
        creation_time: 1_699_999_000u32,
        community_areas: vec![
            AreaTopologySchema {
                area_uuid: "area-a".to_string(),
                name: "Area A".to_string(),
                area_type: "Area".to_string(),
            },
            AreaTopologySchema {
                area_uuid: "area-b".to_string(),
                name: "Area B".to_string(),
                area_type: "Area".to_string(),
            },
        ],
    }
}

fn test_forecasts(market: &MarketTopologySchema) -> Vec<ForecastSchema> {
    vec![
        ForecastSchema {
            area_uuid: "area-a".to_string(),
            community_uuid: "community-1".to_string(),
            time_slot: market.time_slot as u64,
            creation_time: market.creation_time as u64,
            energy_kwh: 12.0,
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: "area-b".to_string(),
            community_uuid: "community-1".to_string(),
            time_slot: market.time_slot as u64,
            creation_time: market.creation_time as u64,
            energy_kwh: -3.0,
            confidence: 0.7,
        },
    ]
}

async fn compile_and_deploy_contract(
    client: Arc<SignerMiddleware<Provider<Ws>, LocalWallet>>,
    source_code: &str,
    contract_name: &str,
) -> Address {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    std::fs::create_dir(&contracts_dir).unwrap();
    let source_path = contracts_dir.join("MockContract.sol");
    {
        let mut file = File::create(&source_path).unwrap();
        file.write_all(source_code.as_bytes()).unwrap();
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
        if err.severity == Severity::Error {
            panic!("Solidity compilation error: {}", err.message);
        }
    }

    let contract_list = output
        .contracts
        .values()
        .flat_map(|inner| inner.iter())
        .find(|(name, _)| *name == contract_name)
        .map(|(_, artifact)| artifact)
        .expect("Contract artifact not found");

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
    factory.deploy(()).unwrap().send().await.unwrap().address()
}

#[tokio::test]
async fn test_publish_orders_calls_evm_order_registry() {
    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await.unwrap();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.20;

        contract MockOrderRegistry {
            struct OrderParams {
                address owner;
                uint64 nonce;
                bytes32 areaUuid;
                bytes32 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
                bool isBid;
            }

            uint256 public placedCount;
            address public lastOwner;
            uint64 public lastNonce;
            bool public lastIsBid;

            function placeOrder(OrderParams calldata params) external {
                placedCount += 1;
                lastOwner = params.owner;
                lastNonce = params.nonce;
                lastIsBid = params.isBid;
            }
        }
    "#;
    let contract_address =
        compile_and_deploy_contract(client.clone(), source, "MockOrderRegistry").await;

    let market = test_market();
    let forecasts = test_forecasts(&market);

    publish_orders(
        ws_endpoint.clone(),
        forecasts,
        market,
        format!("{:?}", contract_address),
        TEST_PRIVATE_KEY.to_string(),
    )
    .await
    .unwrap();

    let mock_contract = MockOrderRegistry::new(contract_address, client.clone());
    assert_eq!(
        mock_contract.placed_count().call().await.unwrap(),
        U256::from(2u64)
    );
    assert_eq!(
        mock_contract.last_owner().call().await.unwrap(),
        anvil.addresses()[0]
    );
    assert!(mock_contract.last_nonce().call().await.unwrap() > 0);
    assert!(!mock_contract.last_is_bid().call().await.unwrap());
}

#[tokio::test]
async fn test_publish_orders_returns_error_for_invalid_contract_address() {
    let market = test_market();
    let forecasts = test_forecasts(&market);

    let err = publish_orders(
        "ws://127.0.0.1:8545".to_string(),
        forecasts,
        market,
        "not-an-address".to_string(),
        TEST_PRIVATE_KEY.to_string(),
    )
    .await
    .unwrap_err();

    assert!(err
        .to_string()
        .to_lowercase()
        .contains("invalid order registry address"));
}

#[tokio::test]
async fn test_publish_orders_returns_error_for_invalid_private_key() {
    let anvil = Anvil::new().spawn();
    let market = test_market();
    let forecasts = test_forecasts(&market);

    let err = publish_orders(
        anvil.ws_endpoint(),
        forecasts,
        market,
        "0x0000000000000000000000000000000000000001".to_string(),
        "not-a-private-key".to_string(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().to_lowercase().contains("invalid"));
}

#[tokio::test]
async fn test_publish_orders_returns_error_when_contract_reverts() {
    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await.unwrap();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));

    let source = r#"
        // SPDX-License-Identifier: MIT
        pragma solidity ^0.8.20;

        contract MockOrderRegistryReverter {
            struct OrderParams {
                address owner;
                uint64 nonce;
                bytes32 areaUuid;
                bytes32 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
                bool isBid;
            }

            function placeOrder(OrderParams calldata) external pure {
                revert("mocked revert");
            }
        }
    "#;
    let contract_address =
        compile_and_deploy_contract(client, source, "MockOrderRegistryReverter").await;
    let market = test_market();
    let forecasts = test_forecasts(&market);

    let err = publish_orders(
        ws_endpoint,
        forecasts,
        market,
        format!("{:?}", contract_address),
        TEST_PRIVATE_KEY.to_string(),
    )
    .await
    .unwrap_err();

    let error_text = err.to_string().to_lowercase();
    assert!(error_text.contains("revert") || error_text.contains("reverted"));
}
