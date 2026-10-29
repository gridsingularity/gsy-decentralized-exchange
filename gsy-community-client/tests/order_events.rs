mod common;

use common::compile_and_deploy_contract;
use ethers::{prelude::*, utils::Anvil, utils::AnvilInstance};
use gsy_community_client::order_events::{
    order_params, start_order_event_subscriber, OrderEventHandler, OrderRegistryClient,
    PlaceOrderError,
};
use primitives::db_api_schema::orders::DbOrderSchema;
use primitives::ewds::dto::EwdsEventEnvelope;
use primitives::ewds::dto::{EwdsOrderDto, EwdsSendMessageDto};
use primitives::ewds::{
    is_invalid_event, EwdsClient, EwdsClientConfig, EwdsEventTopicConfig, EwdsEventType,
    EwdsTopicConfig,
};
use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
use primitives::utils::{parse_uuid_or_hex_bytes16, rfc3339_to_epoch};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

abigen!(
    MockOrderRegistry,
    r#"[
        struct OrderParams { bytes16 orderId; bytes16 createdBy; bytes16 marketId; uint64 timeSlot; uint64 creationTime; uint64 energy; uint64 energyRate; uint8 energySourcePreference; uint8 energyType; bool isBid; bytes16 preferredTradingPartner; uint64 preferredEnergyRate; }
        function placedCount() external view returns (uint256)
        function placedOrder(uint256 index) external view returns (OrderParams)
        function setClosedMarket(bytes16 marketId) external
        function closedMarketId() external view returns (bytes16)
    ]"#
);

/// Mimics `OrderRegistry.placeOrder` and `getStatus`, records every placed order and reports
/// one market as closed.
const MOCK_ORDER_REGISTRY: &str = r#"
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

        error MarketClosed();
        error OrderAlreadyExists();

        bytes16 public closedMarketId;
        mapping(bytes16 => uint8) private orderStatus;
        OrderParams[] private placed;

        function setClosedMarket(bytes16 marketId) external {
            closedMarketId = marketId;
        }

        function placeOrder(OrderParams calldata params) external {
            if (params.marketId == closedMarketId) revert MarketClosed();
            if (orderStatus[params.orderId] != 0) revert OrderAlreadyExists();
            orderStatus[params.orderId] = 1;
            placed.push(params);
        }

        function getStatus(bytes16 orderId) external view returns (uint8) {
            return orderStatus[orderId];
        }

        function placedCount() external view returns (uint256) {
            return placed.length;
        }

        function placedOrder(uint256 index) external view returns (OrderParams memory) {
            return placed[index];
        }
    }
"#;

const TEST_PRIVATE_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const BID_ID: &str = "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f";
const OFFER_ID: &str = "9a1b2c3d-4e5f-4a6b-8c7d-0e1f2a3b4c5d";
const OPEN_MARKET: &str = "0x11111111111111111111111111111111";
const CLOSED_MARKET: &str = "0x22222222222222222222222222222222";
/// On-chain IDs the mocked ID service maps `owner-1` and `owner-2` to.
const OWNER_1_ONCHAIN: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER_2_ONCHAIN: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// 2026-09-30T10:00:00Z and 2026-09-30T09:40:12Z.
const TIME_SLOT: u64 = 1_790_762_400;
const CREATION_TIME: u64 = 1_790_761_212;

type TestSigner = SignerMiddleware<Provider<Ws>, LocalWallet>;

struct TestChain {
    // Keeps the node running for the duration of the test.
    _anvil: AnvilInstance,
    ws_endpoint: String,
    contract_address: Address,
    contract: MockOrderRegistry<TestSigner>,
}

async fn deploy_mock_order_registry() -> TestChain {
    let anvil = Anvil::new().spawn();
    let ws_endpoint = anvil.ws_endpoint();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    let provider = Provider::<Ws>::connect(&ws_endpoint).await.unwrap();
    let client = Arc::new(SignerMiddleware::new(
        provider,
        wallet.with_chain_id(anvil.chain_id()),
    ));
    let contract_address =
        compile_and_deploy_contract(client.clone(), MOCK_ORDER_REGISTRY, "MockOrderRegistry").await;
    let contract = MockOrderRegistry::new(contract_address, client);
    contract
        .set_closed_market(bytes16(CLOSED_MARKET))
        .send()
        .await
        .unwrap()
        .await
        .unwrap();

    TestChain {
        _anvil: anvil,
        ws_endpoint,
        contract_address,
        contract,
    }
}

/// Mocks the ID service and the gateway's outbound messages endpoint on one server.
async fn mock_id_service() -> MockServer {
    let server = MockServer::start().await;
    for (offchain_id, onchain_id) in [("owner-1", OWNER_1_ONCHAIN), ("owner-2", OWNER_2_ONCHAIN)] {
        Mock::given(method("POST"))
            .and(path("/ids"))
            .and(query_param("offchain_id", offchain_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "offchain_id": offchain_id,
                "onchain_id": onchain_id,
                "creation_time": 0,
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/ids"))
        .respond_with(ResponseTemplate::new(404))
        .with_priority(10)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "recipients": {"sent": 1, "failed": 0, "total": 1}
        })))
        .mount(&server)
        .await;
    server
}

async fn handler(chain: &TestChain, id_service: &MockServer) -> OrderEventHandler {
    let order_registry = OrderRegistryClient::connect(
        chain.ws_endpoint.as_str(),
        format!("{:?}", chain.contract_address).as_str(),
        TEST_PRIVATE_KEY,
    )
    .await
    .unwrap();
    OrderEventHandler::new(
        OffchainStorageClient::new(
            OffchainStorageTransport::Http,
            id_service.uri(),
            "EWDS_COMMUNITY_CLIENT_ID",
            "gsycommunityclient",
        ),
        order_registry,
        EwdsClient::new(EwdsClientConfig {
            gateway_base: id_service.uri(),
            request_fqcn: "gsy.requests.pub".to_string(),
            response_fqcn: "gsy.responses.sub".to_string(),
            topic_owner: "test.owner".to_string(),
            topic_version: "1.0.0".to_string(),
            consumer_client_id: "gsycommunityclient".to_string(),
            timeout_ms: 1_000,
            poll_interval_ms: 10,
            empty_response_grace_ms: 0,
            topics: EwdsTopicConfig::default(),
            event_publish_fqcn: "gsy.events.pub".to_string(),
            event_subscribe_fqcn: "gsy.events.sub".to_string(),
            event_batch_size: 50,
            event_poll_interval_ms: 10,
            event_handle_attempts: 3,
            event_retry_delay_ms: 10,
            event_topics: EwdsEventTopicConfig::default(),
        }),
    )
}

async fn rejection_events(server: &MockServer) -> Vec<EwdsEventEnvelope<Vec<EwdsOrderDto>>> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter_map(|request| {
            // Rejections must never be written to the order database through HTTP.
            assert!(matches!(request.url.path(), "/ids" | "/api/v2/messages"));
            if request.url.path() == "/ids" {
                return None;
            }
            let sent: EwdsSendMessageDto = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(sent.fqcn, "gsy.events.pub");
            assert_eq!(sent.topic_name, "order");
            let event: EwdsEventEnvelope<Vec<EwdsOrderDto>> =
                serde_json::from_str(&sent.payload).unwrap();
            assert_eq!(event.event_type, EwdsEventType::OrderStatus);
            assert_eq!(event.event_id, sent.transaction_id);
            Some(event)
        })
        .collect()
}

fn assert_rejection(event: &EwdsEventEnvelope<Vec<EwdsOrderDto>>, original: Value, reason: &str) {
    assert_eq!(event.data.len(), 1);
    let mut expected: EwdsOrderDto = serde_json::from_value(original).unwrap();
    expected.order_status = "rejected".to_string();
    expected.reject_reason = Some(reason.to_string());
    expected.updated_at = Some(event.occurred_at.clone());
    assert!(rfc3339_to_epoch(&event.occurred_at).is_ok());
    assert_ne!(event.occurred_at, expected.creation_time);
    assert_eq!(
        serde_json::to_value(&event.data[0]).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

fn bytes16(value: &str) -> [u8; 16] {
    parse_uuid_or_hex_bytes16(value).unwrap()
}

fn bid() -> Value {
    json!({
        "orderId": BID_ID,
        "marketId": OPEN_MARKET,
        "orderType": "bid",
        "orderStatus": "submitted",
        "timeSlot": "2026-09-30T10:00:00Z",
        "quantity": 1.5,
        "priceLimit": 0.3,
        "createdBy": "owner-1",
        "creationTime": "2026-09-30T09:40:12Z",
        "energySourcePreference": "GREEN",
        "preferredTradingPartner": "owner-2",
        "preferredEnergyRate": 0.25,
    })
}

fn offer() -> Value {
    json!({
        "orderId": OFFER_ID,
        "marketId": OPEN_MARKET,
        "orderType": "offer",
        "orderStatus": "submitted",
        "timeSlot": "2026-09-30T10:00:00Z",
        "quantity": 2.0,
        "priceLimit": 0.2,
        "createdBy": "owner-2",
        "creationTime": "2026-09-30T09:40:12Z",
        "energyType": "PV",
        "preferredTradingPartner": "owner-1",
        "preferredEnergyRate": 0.15,
    })
}

fn event(event_id: &str, orders: Vec<Value>) -> EwdsEventEnvelope<Value> {
    EwdsEventEnvelope {
        event_id: event_id.to_string(),
        event_type: EwdsEventType::Order,
        occurred_at: "2026-09-30T09:40:13Z".to_string(),
        data: Value::Array(orders),
    }
}

async fn placed_orders(chain: &TestChain) -> Vec<OrderParams> {
    let count = chain.contract.placed_count().call().await.unwrap().as_u64();
    let mut orders = Vec::new();
    for index in 0..count {
        let order = chain
            .contract
            .placed_order(U256::from(index))
            .call()
            .await
            .unwrap();
        orders.push(OrderParams {
            order_id: order.0,
            created_by: order.1,
            market_id: order.2,
            time_slot: order.3,
            creation_time: order.4,
            energy: order.5,
            energy_rate: order.6,
            energy_source_preference: order.7,
            energy_type: order.8,
            is_bid: order.9,
            preferred_trading_partner: order.10,
            preferred_energy_rate: order.11,
        });
    }
    orders
}

/// Waits for orders submitted by the background subscriber.
async fn wait_for_placed_orders(chain: &TestChain, expected: usize) -> Vec<OrderParams> {
    for _ in 0..50 {
        let orders = placed_orders(chain).await;
        if orders.len() >= expected {
            return orders;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    placed_orders(chain).await
}

/// How many transactions the test wallet sent, mined or pending. Deploying the mock and
/// closing its market take two.
async fn sent_transactions(chain: &TestChain) -> u64 {
    let client = chain.contract.client();
    client
        .get_transaction_count(client.address(), Some(BlockNumber::Pending.into()))
        .await
        .unwrap()
        .as_u64()
}

#[tokio::test]
async fn places_a_bid_and_an_offer_with_their_contract_params() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;

    handler(&chain, &id_service)
        .await
        .handle(event("event-1", vec![bid(), offer()]))
        .await
        .unwrap();

    assert_eq!(
        placed_orders(&chain).await,
        vec![
            OrderParams {
                order_id: bytes16(BID_ID),
                created_by: bytes16(OWNER_1_ONCHAIN),
                market_id: bytes16(OPEN_MARKET),
                time_slot: TIME_SLOT,
                creation_time: CREATION_TIME,
                energy: 15_000,
                energy_rate: 3_000,
                energy_source_preference: 1, // GREEN
                energy_type: 0,
                is_bid: true,
                preferred_trading_partner: bytes16(OWNER_2_ONCHAIN),
                preferred_energy_rate: 2_500,
            },
            OrderParams {
                order_id: bytes16(OFFER_ID),
                created_by: bytes16(OWNER_2_ONCHAIN),
                market_id: bytes16(OPEN_MARKET),
                time_slot: TIME_SLOT,
                creation_time: CREATION_TIME,
                energy: 20_000,
                energy_rate: 2_000,
                energy_source_preference: 0,
                energy_type: 2, // PV
                is_bid: false,
                preferred_trading_partner: bytes16(OWNER_1_ONCHAIN),
                preferred_energy_rate: 1_500,
            },
        ]
    );
    assert!(rejection_events(&id_service).await.is_empty());
}

#[tokio::test]
async fn skips_orders_that_are_already_placed() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let handler = handler(&chain, &id_service).await;

    handler.handle(event("event-1", vec![bid()])).await.unwrap();
    wait_for_placed_orders(&chain, 1).await;
    // A resend under a new event ID sends only the new order.
    handler
        .handle(event("event-2", vec![bid(), offer()]))
        .await
        .unwrap();

    let order_ids = wait_for_placed_orders(&chain, 2)
        .await
        .into_iter()
        .map(|order| order.order_id)
        .collect::<Vec<_>>();
    assert_eq!(order_ids, vec![bytes16(BID_ID), bytes16(OFFER_ID)]);
    assert_eq!(sent_transactions(&chain).await, 2 + 2);
    assert!(rejection_events(&id_service).await.is_empty());
}

#[tokio::test]
async fn places_the_other_orders_when_the_contract_rejects_one() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let mut bid_in_closed_market = bid();
    bid_in_closed_market["marketId"] = json!(CLOSED_MARKET);

    handler(&chain, &id_service)
        .await
        .handle(event(
            "event-1",
            vec![bid_in_closed_market.clone(), offer()],
        ))
        .await
        .unwrap();

    let order_ids = wait_for_placed_orders(&chain, 1)
        .await
        .into_iter()
        .map(|order| order.order_id)
        .collect::<Vec<_>>();
    assert_eq!(order_ids, vec![bytes16(OFFER_ID)]);
    // The rejected order was never sent.
    assert_eq!(sent_transactions(&chain).await, 2 + 1);
    let notifications = rejection_events(&id_service).await;
    assert_eq!(notifications.len(), 1);
    assert_rejection(&notifications[0], bid_in_closed_market, "MarketClosed");
}

#[tokio::test]
async fn fails_the_event_when_the_node_cannot_be_reached() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let handler = handler(&chain, &id_service).await;
    // Stops the node.
    drop(chain);

    let error = tokio::time::timeout(
        Duration::from_secs(30),
        handler.handle(event("event-1", vec![bid(), offer()])),
    )
    .await
    .expect("the handler did not give up on the unreachable node")
    .unwrap_err();

    // The event worker retries the event, so it must not be marked as invalid.
    assert!(!is_invalid_event(&error));
    assert!(format!("{error:#}").contains("2 of 2 orders"), "{error:#}");
    assert!(rejection_events(&id_service).await.is_empty());
}

#[tokio::test]
async fn rejects_the_whole_event_when_one_order_is_invalid() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let handler = handler(&chain, &id_service).await;

    // An ID the ID service doesn't know yet may be registered later, so that event is retried;
    // invalid data is not.
    for (field, value, expected, invalid_data) in [
        ("orderId", json!("order-1"), "is not a UUID", true),
        ("orderStatus", json!("cancelled"), "orderStatus", true),
        ("quantity", json!(0.0), "quantity", true),
        ("quantity", json!(-1.0), "quantity", true),
        ("priceLimit", json!(-0.1), "priceLimit", true),
        ("energyType", json!("COAL"), "COAL", true),
        ("createdBy", json!("unknown-owner"), "createdBy", false),
        (
            "preferredTradingPartner",
            json!("unknown-owner"),
            "preferredTradingPartner",
            false,
        ),
        ("marketId", json!("market-1"), "marketId", true),
    ] {
        let mut invalid = offer();
        invalid[field] = value;

        let error = handler
            .handle(event("event-1", vec![bid(), invalid]))
            .await
            .unwrap_err();

        assert_eq!(is_invalid_event(&error), invalid_data, "{field}: {error:#}");
        let error = format!("{error:#}");
        assert!(error.contains("index 1"), "{field}: {error}");
        assert!(error.contains(expected), "{field}: {error}");
    }
    assert!(placed_orders(&chain).await.is_empty());
    assert_eq!(sent_transactions(&chain).await, 2);
    assert!(rejection_events(&id_service).await.is_empty());
}

#[tokio::test]
async fn rejection_notification_failure_does_not_retry_placement_or_block_other_orders() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("invalid topic"))
        .with_priority(1)
        .mount(&id_service)
        .await;
    let mut rejected = bid();
    rejected["marketId"] = json!(CLOSED_MARKET);
    handler(&chain, &id_service)
        .await
        .handle(event("event-1", vec![rejected.clone(), offer()]))
        .await
        .unwrap();

    let orders = placed_orders(&chain).await;
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].order_id, bytes16(OFFER_ID));
    assert_eq!(sent_transactions(&chain).await, 3);
    let notifications = rejection_events(&id_service).await;
    assert_eq!(notifications.len(), 1);
    assert_rejection(&notifications[0], rejected, "MarketClosed");
}

#[tokio::test]
async fn notifies_rejection_only_after_a_failed_mined_receipt() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let handler = handler(&chain, &id_service).await;
    let client = chain.contract.client();
    let provider = client.provider();
    provider
        .request::<_, Value>("evm_setAutomine", [false])
        .await
        .unwrap();
    let initial_nonce = sent_transactions(&chain).await;
    let handling =
        tokio::spawn(async move { handler.handle(event("event-1", vec![offer()])).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        while sent_transactions(&chain).await == initial_nonce {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("order was not submitted");
    assert!(!handling.is_finished());
    assert!(rejection_events(&id_service).await.is_empty());

    // Close the market after gas estimation, but before the transaction executes.
    let mut closed_market = [0u8; 32];
    closed_market[16..].copy_from_slice(&bytes16(OPEN_MARKET));
    provider
        .request::<_, Value>(
            "anvil_setStorageAt",
            (
                chain.contract_address,
                H256::zero(),
                H256::from(closed_market),
            ),
        )
        .await
        .unwrap();
    provider.request::<_, Value>("evm_mine", ()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(20), handling)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert!(placed_orders(&chain).await.is_empty());
    assert_eq!(sent_transactions(&chain).await, initial_nonce + 1);
    let notifications = rejection_events(&id_service).await;
    assert_eq!(notifications.len(), 1);
    let reason = notifications[0].data[0].reject_reason.as_deref().unwrap();
    assert!(reason.contains("reverted"));
    assert_rejection(&notifications[0], offer(), reason);
}

#[tokio::test]
async fn order_registry_client_waits_for_mining_and_detects_execution_reverts() {
    for revert_after_submission in [false, true] {
        let chain = deploy_mock_order_registry().await;
        let registry = OrderRegistryClient::connect(
            &chain.ws_endpoint,
            &format!("{:?}", chain.contract_address),
            TEST_PRIVATE_KEY,
        )
        .await
        .unwrap();
        let mut order =
            DbOrderSchema::try_from(serde_json::from_value::<EwdsOrderDto>(offer()).unwrap())
                .unwrap();
        order.created_by = OWNER_2_ONCHAIN.to_string();
        order.requirements = None;
        let params = order_params(&order).unwrap();
        let client = chain.contract.client();
        let provider = client.provider();
        provider
            .request::<_, Value>("evm_setAutomine", [false])
            .await
            .unwrap();
        let initial_nonce = sent_transactions(&chain).await;
        let placement = tokio::spawn(async move { registry.place_order(params).await });
        tokio::time::timeout(Duration::from_secs(10), async {
            while sent_transactions(&chain).await == initial_nonce {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("placeOrder transaction was not submitted");
        // Let the caller process the node's acknowledgement before checking it is still waiting.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !placement.is_finished(),
            "pending transaction was reported as placed"
        );
        assert!(placed_orders(&chain).await.is_empty());

        if revert_after_submission {
            // Change the mock's closedMarketId (slot 0) after estimation/send but before mining.
            // This deterministically reproduces an order reverting when its market closes.
            let mut closed_market = [0u8; 32];
            closed_market[16..].copy_from_slice(&bytes16(OPEN_MARKET));
            provider
                .request::<_, Value>(
                    "anvil_setStorageAt",
                    (
                        chain.contract_address,
                        H256::zero(),
                        H256::from(closed_market),
                    ),
                )
                .await
                .unwrap();
            assert_eq!(
                chain.contract.closed_market_id().call().await.unwrap(),
                bytes16(OPEN_MARKET)
            );
        }
        provider.request::<_, Value>("evm_mine", ()).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(20), placement)
            .await
            .expect("placeOrder did not process the mined receipt")
            .unwrap();
        if revert_after_submission {
            assert!(
                matches!(result, Err(PlaceOrderError::Rejected(ref reason)) if reason.contains("reverted")),
                "expected a mined rejection, got {:?}",
                result
            );
            assert!(placed_orders(&chain).await.is_empty());
        } else {
            let receipt = provider
                .get_transaction_receipt(result.unwrap())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(receipt.status, Some(U64::from(1)));
            assert_eq!(placed_orders(&chain).await.len(), 1);
        }
        assert_eq!(sent_transactions(&chain).await, initial_nonce + 1);
    }
}

#[tokio::test]
async fn order_registry_client_reports_why_the_contract_rejected_an_order() {
    let chain = deploy_mock_order_registry().await;
    let order_registry = OrderRegistryClient::connect(
        chain.ws_endpoint.as_str(),
        format!("{:?}", chain.contract_address).as_str(),
        TEST_PRIVATE_KEY,
    )
    .await
    .unwrap();
    let mut order: DbOrderSchema =
        DbOrderSchema::try_from(serde_json::from_value::<EwdsOrderDto>(offer()).unwrap()).unwrap();
    order.created_by = OWNER_2_ONCHAIN.to_string();
    order.requirements = None;
    let params = order_params(&order).unwrap();

    assert!(!order_registry.is_placed(params.0).await.unwrap());
    order_registry.place_order(params).await.unwrap();
    wait_for_placed_orders(&chain, 1).await;
    assert!(order_registry.is_placed(params.0).await.unwrap());
    match order_registry.place_order(params).await {
        Err(PlaceOrderError::Rejected(reason)) => assert_eq!(reason, "OrderAlreadyExists"),
        other => panic!("expected OrderAlreadyExists, got {:?}", other),
    }

    order.order_id = BID_ID.to_string();
    order.market_id = CLOSED_MARKET.to_string();
    match order_registry
        .place_order(order_params(&order).unwrap())
        .await
    {
        Err(PlaceOrderError::Rejected(reason)) => assert_eq!(reason, "MarketClosed"),
        other => panic!("expected MarketClosed, got {:?}", other),
    }
}

#[tokio::test]
async fn subscriber_places_submissions_but_not_status_notifications_from_the_order_topic() {
    let chain = deploy_mock_order_registry().await;
    let id_service = mock_id_service().await;
    let gateway = MockServer::start().await;
    // A Submitted notification looks like a valid submission except for its event type.
    // Place it first so an incorrect dispatch would submit the offer before the real bid.
    let notification = EwdsEventEnvelope {
        event_type: EwdsEventType::OrderStatus,
        ..event("status-1", vec![offer()])
    };
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.intelligent.events.sub"))
        .and(query_param("topicName", "order"))
        .and(query_param("clientId", "gsycommunityclientorder"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"payload": serde_json::to_string(&notification).unwrap()},
            {"payload": serde_json::to_string(&event("event-1", vec![bid()])).unwrap()},
        ])))
        .mount(&gateway)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(&gateway)
        .await;

    // The only test in this binary that reads the environment.
    for key in [
        "EWDS_EVENT_SUBSCRIBE_FQCN",
        "EWDS_ORDER_EVENT_TOPIC",
        "EWDS_COMMUNITY_CLIENT_ID",
        "EWDS_RESPONSE_CLIENT_ID",
        "OFFCHAIN_STORAGE_TRANSPORT",
    ] {
        std::env::remove_var(key);
    }
    std::env::set_var("EWDS_GATEWAY_URL", gateway.uri());
    std::env::set_var("OFFCHAIN_STORAGE_URL", id_service.uri());
    std::env::set_var("EVM_NODE_URL", chain.ws_endpoint.as_str());
    std::env::set_var(
        "ORDER_REGISTRY_ADDRESS",
        format!("{:?}", chain.contract_address),
    );
    std::env::set_var("COMMUNITY_CLIENT_PRIVATE_KEY", TEST_PRIVATE_KEY);

    let subscriber = tokio::spawn(start_order_event_subscriber());
    let mut orders = Vec::new();
    for _ in 0..100 {
        orders = placed_orders(&chain).await;
        if !orders.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    subscriber.abort();

    let order_ids = orders
        .into_iter()
        .map(|order| order.order_id)
        .collect::<Vec<_>>();
    assert_eq!(order_ids, vec![bytes16(BID_ID)]);
}

mod validate_order {
    use super::*;
    use gsy_community_client::order_events::validate_order;
    use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f".to_string(),
            status: OrderStatus::Submitted,
            status_updated_at: None,
            order_type: OrderEnum::Bid,
            area_uuid: "owner-1".to_string(),
            market_id: format!("0x{}", "11".repeat(16)),
            time_slot: 1_790_762_400,
            creation_time: 1_790_761_212,
            energy_kWh: 1.5,
            energy_rate: 0.3,
            created_by: "owner-1".to_string(),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn accepts_a_valid_order() {
        assert_eq!(validate_order(order()).unwrap(), order());
    }

    #[test]
    fn accepts_a_price_limit_of_zero() {
        assert!(validate_order(DbOrderSchema {
            energy_rate: 0.0,
            ..order()
        })
        .is_ok());
    }

    #[test]
    fn rejects_invalid_orders() {
        for (invalid, expected) in [
            (
                DbOrderSchema {
                    order_id: "order-1".to_string(),
                    ..order()
                },
                "is not a UUID",
            ),
            (
                DbOrderSchema {
                    status: OrderStatus::Cancelled,
                    ..order()
                },
                "orderStatus",
            ),
            (
                DbOrderSchema {
                    energy_kWh: 0.0,
                    ..order()
                },
                "quantity",
            ),
            (
                DbOrderSchema {
                    energy_kWh: -1.0,
                    ..order()
                },
                "quantity",
            ),
            (
                DbOrderSchema {
                    energy_rate: -0.1,
                    ..order()
                },
                "priceLimit",
            ),
        ] {
            let error = validate_order(invalid).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}

mod order_params {
    use super::*;
    use primitives::db_api_schema::orders::{
        DbAttributes, DbRequirements, EnergyType, OrderEnum, OrderStatus,
    };
    use primitives::utils::bytes16_to_hex;

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f".to_string(),
            status: OrderStatus::Submitted,
            status_updated_at: None,
            order_type: OrderEnum::Offer,
            area_uuid: format!("0x{}", "aa".repeat(16)),
            market_id: format!("0x{}", "11".repeat(16)),
            time_slot: 1_790_762_400,
            creation_time: 1_790_761_212,
            energy_kWh: 0.30005,
            energy_rate: 0.07,
            created_by: format!("0x{}", "aa".repeat(16)),
            requirements: None,
            attributes: Some(DbAttributes {
                energy_type: EnergyType::Battery,
            }),
        }
    }

    #[test]
    fn order_params_keeps_the_uuid_bytes_and_scales_energy_and_rate() {
        let params = order_params(&order()).unwrap();

        assert_eq!(
            bytes16_to_hex(params.0),
            "0x3f2c6d1e8a4b4c7d9e2f1a5b6c7d8e9f"
        );
        assert_eq!(params.1, [0xaa; 16]);
        assert_eq!(params.2, [0x11; 16]);
        assert_eq!((params.3, params.4), (1_790_762_400, 1_790_761_212));
        // Rounded, not truncated.
        assert_eq!((params.5, params.6), (3_001, 700));
        assert_eq!((params.7, params.8), (0, 5));
        assert!(!params.9);
        assert_eq!((params.10, params.11), ([0; 16], 0));
    }

    #[test]
    fn order_params_rejects_ids_that_are_not_on_chain_ids() {
        for (invalid, expected) in [
            (
                DbOrderSchema {
                    created_by: "owner-1".to_string(),
                    ..order()
                },
                "createdBy",
            ),
            (
                DbOrderSchema {
                    market_id: format!("0x{}", "00".repeat(16)),
                    ..order()
                },
                "marketId",
            ),
            (
                DbOrderSchema {
                    order_type: OrderEnum::Bid,
                    requirements: Some(DbRequirements {
                        trading_partner_id: Some("owner-2".to_string()),
                        energy_type: None,
                        preferred_energy_rate: None,
                    }),
                    attributes: None,
                    ..order()
                },
                "resolved through the ID service",
            ),
        ] {
            let error = order_params(&invalid).err().unwrap().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}
