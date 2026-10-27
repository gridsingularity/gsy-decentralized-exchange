mod common;
#[path = "common/facilities.rs"]
mod facilities;

use common::compile_and_deploy_contract;
use ethers::{prelude::*, utils::Anvil};
use gsy_community_client::node_connector::orders::publish_orders;
use primitives::db_api_schema::market::MarketSchema;
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::utils::parse_uuid_or_hex_bytes16;
use primitives::{MarketType, MatchingAlgorithm};
use std::sync::Arc;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

abigen!(
    MockOrderRegistry,
    r#"[
        function placedCount() external view returns (uint256)
        function lastCreatedBy() external view returns (bytes16)
        function lastMarketId() external view returns (bytes16)
        function lastEnergySourcePreference() external view returns (uint8)
        function lastEnergyType() external view returns (uint8)
        function lastIsBid() external view returns (bool)
        function lastPreferredTradingPartner() external view returns (bytes16)
        function lastPreferredEnergyRate() external view returns (uint64)
    ]"#
);

const TEST_PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

async fn setup_facilities_mock() -> MockServer {
    facilities::setup_facilities_mock(["area-a", "area-b"]).await
}

fn test_market() -> MarketSchema {
    MarketSchema {
        market_id: format!("0x{}", "11".repeat(16)),
        community_id: "community-1".to_string(),
        opening_time: "00000000002700000000".to_string(),
        closing_time: "00000000002800000000".to_string(),
        delivery_start_time: "00000000001700000000".to_string(),
        delivery_end_time: "00000000001800000000".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: "00000000001699999000".to_string(),
    }
}

fn test_forecasts() -> Vec<ForecastSchema> {
    vec![
        ForecastSchema {
            facility_id: "area-a".to_string(),
            community_uuid: "community-1".to_string(),
            time_slot: 1_700_000_000,
            creation_time: 1_699_999_000,
            energy_kwh: 12.0,
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area-b".to_string(),
            community_uuid: "community-1".to_string(),
            time_slot: 1_700_000_000,
            creation_time: 1_699_999_000,
            energy_kwh: -3.0,
            confidence: 0.7,
        },
    ]
}

#[tokio::test]
#[serial_test::serial]
async fn test_publish_orders_calls_evm_order_registry() {
    let facilities_server = setup_facilities_mock().await;
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
                bytes16 orderId;
                bytes16 createdBy;
                bytes16 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
                uint8 energySourcePreference;
                uint8 energyType;
                bool isBid;
                bytes16 preferredTradingPartner;
                uint64 preferredEnergyRate;
            }

            uint256 public placedCount;
            bytes16 public lastCreatedBy;
            bytes16 public lastMarketId;
            uint8 public lastEnergySourcePreference;
            uint8 public lastEnergyType;
            bool public lastIsBid;
            bytes16 public lastPreferredTradingPartner;
            uint64 public lastPreferredEnergyRate;

            function placeOrder(OrderParams calldata params) external {
                placedCount += 1;
                lastCreatedBy = params.createdBy;
                lastMarketId = params.marketId;
                lastEnergySourcePreference = params.energySourcePreference;
                lastEnergyType = params.energyType;
                lastIsBid = params.isBid;
                lastPreferredTradingPartner = params.preferredTradingPartner;
                lastPreferredEnergyRate = params.preferredEnergyRate;
            }
        }
    "#;
    let contract_address =
        compile_and_deploy_contract(client.clone(), source, "MockOrderRegistry").await;
    let mock_contract = MockOrderRegistry::new(contract_address, client.clone());

    // Validation or ID-service failures must abort preparation before the first transaction.
    for endpoint in ["/facilities/validate", "/ids"] {
        let mut failure = Mock::given(method("POST")).and(path(endpoint));
        if endpoint == "/ids" {
            failure = failure.and(query_param("offchain_id", "area-b"));
        }
        failure
            .respond_with(ResponseTemplate::new(500))
            .with_priority(1)
            .up_to_n_times(1)
            .mount(&facilities_server)
            .await;
        assert!(publish_orders(
            ws_endpoint.clone(),
            test_forecasts(),
            test_market(),
            format!("{:?}", contract_address),
            TEST_PRIVATE_KEY.to_string(),
        )
        .await
        .is_err());
        assert_eq!(
            mock_contract.placed_count().call().await.unwrap(),
            U256::zero()
        );
    }

    let market = test_market();

    let mut forecasts = test_forecasts();
    let mut denied = forecasts[0].clone();
    denied.facility_id = "missing-facility".to_string();
    forecasts.insert(1, denied);

    publish_orders(
        ws_endpoint.clone(),
        forecasts,
        market,
        format!("{:?}", contract_address),
        TEST_PRIVATE_KEY.to_string(),
    )
    .await
    .unwrap();

    assert_eq!(
        mock_contract.placed_count().call().await.unwrap(),
        U256::from(2u64)
    );
    assert_eq!(
        mock_contract.last_created_by().call().await.unwrap(),
        parse_uuid_or_hex_bytes16(facilities::FACILITY_2_ONCHAIN).unwrap()
    );
    assert_eq!(
        mock_contract.last_market_id().call().await.unwrap(),
        parse_uuid_or_hex_bytes16("0x11111111111111111111111111111111")
            .expect("failed to parse uuid")
    );
    assert_eq!(
        mock_contract
            .last_energy_source_preference()
            .call()
            .await
            .unwrap(),
        0u8
    );
    assert_eq!(mock_contract.last_energy_type().call().await.unwrap(), 0u8);
    assert!(!mock_contract.last_is_bid().call().await.unwrap());
    assert_eq!(
        mock_contract
            .last_preferred_trading_partner()
            .call()
            .await
            .unwrap(),
        [0; 16]
    );
    assert_eq!(
        mock_contract
            .last_preferred_energy_rate()
            .call()
            .await
            .unwrap(),
        0
    );

    // A later cross-community denial must not submit another order, even with an ID mapping.
    Mock::given(method("POST"))
        .and(path("/facilities/validate"))
        .respond_with(|request: &Request| {
            let pairs: Vec<serde_json::Value> = serde_json::from_slice(&request.body).unwrap();
            ResponseTemplate::new(200).set_body_json(
                pairs
                    .into_iter()
                    .map(|pair| {
                        serde_json::json!({
                            "facility_id": pair["facility_id"], "market_id": pair["market_id"],
                            "eligible": false, "reason": "site_not_in_community",
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .with_priority(1)
        .mount(&facilities_server)
        .await;
    publish_orders(
        ws_endpoint,
        test_forecasts(),
        test_market(),
        format!("{:?}", contract_address),
        TEST_PRIVATE_KEY.to_string(),
    )
    .await
    .unwrap();
    assert_eq!(
        mock_contract.placed_count().call().await.unwrap(),
        U256::from(2)
    );
    assert!(facilities_server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|request| matches!(request.url.path(), "/facilities/validate" | "/ids")));
}

#[tokio::test]
async fn test_publish_orders_returns_error_for_invalid_contract_address() {
    let market = test_market();
    let forecasts = test_forecasts();

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
    let forecasts = test_forecasts();

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
#[serial_test::serial]
async fn test_publish_orders_returns_error_when_contract_reverts() {
    let _facilities_server = setup_facilities_mock().await;

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
                bytes16 orderId;
                bytes16 createdBy;
                bytes16 marketId;
                uint64 timeSlot;
                uint64 creationTime;
                uint64 energy;
                uint64 energyRate;
                uint8 energySourcePreference;
                uint8 energyType;
                bool isBid;
                bytes16 preferredTradingPartner;
                uint64 preferredEnergyRate;
            }

            function placeOrder(OrderParams calldata) external pure {
                revert("mocked revert");
            }
        }
    "#;
    let contract_address =
        compile_and_deploy_contract(client, source, "MockOrderRegistryReverter").await;
    let market = test_market();
    let forecasts = test_forecasts();

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
