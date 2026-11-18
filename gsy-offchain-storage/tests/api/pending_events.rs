use crate::helpers::{init_app, stop_app, TestApp};
use ethers::{
    abi::{encode, Token},
    prelude::*,
};
use gsy_ethers_listener::{
    ChainEvent, DeliveryVerifiedFilter, GsyEventHandler, OrderCancelledFilter, OrderPlacedFilter,
    PenaltyRecordedFilter, TradeSettledFilter,
};
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use mongodb::bson::{doc, Document};
use primitives::{
    db_api_schema::{orders::OrderStatus, trades::TradeStatus},
    utils::{bytes16_to_hex, derive_onchain_id, parse_uuid_or_hex_bytes16},
};

const STREAM: &str = "test-chain-and-contracts";

fn hash(id: &str) -> [u8; 16] {
    parse_uuid_or_hex_bytes16(&derive_onchain_id(id)).unwrap()
}

fn topic(id: [u8; 16]) -> H256 {
    let mut padded = [0; 32];
    padded[..16].copy_from_slice(&id);
    H256(padded)
}

fn event(number: u64, signature: H256, indexed: &[[u8; 16]], data: Vec<Token>) -> ChainEvent {
    ChainEvent {
        stream_id: STREAM.into(),
        block_timestamp: 900 + number,
        log: Log {
            topics: std::iter::once(signature)
                .chain(indexed.iter().copied().map(topic))
                .collect(),
            data: encode(&data).into(),
            block_hash: Some(H256::from_low_u64_be(number)),
            block_number: Some(number.into()),
            transaction_hash: Some(H256::from_low_u64_be(number)),
            transaction_index: Some(0.into()),
            log_index: Some(0.into()),
            ..Default::default()
        },
    }
}

fn uint(value: u64) -> Token {
    Token::Uint(value.into())
}
fn bytes(value: [u8; 16]) -> Token {
    Token::FixedBytes(value.to_vec())
}

fn placed(
    number: u64,
    id: u8,
    market: u8,
    owner: &str,
    partner: Option<&str>,
    bid: bool,
) -> ChainEvent {
    event(
        number,
        OrderPlacedFilter::signature(),
        &[[id; 16], hash(owner), [market; 16]],
        vec![
            uint(1000),
            uint(890),
            uint(10_000),
            uint(50_000),
            uint(0),
            uint(0),
            Token::Bool(bid),
            bytes(partner.map(hash).unwrap_or([0; 16])),
            uint(0),
        ],
    )
}

fn handler(app: &TestApp) -> OffchainStorageEvmHandler {
    OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    }
}

async fn register(app: &TestApp, id: &str) {
    app.db_wrapper.ids().get_or_create(id.into()).await.unwrap();
}

async fn pending(app: &TestApp) -> u64 {
    app.db_wrapper
        .collection::<Document>("evm_pending_events")
        .count_documents(doc! {})
        .await
        .unwrap()
}

#[tokio::test]
async fn missing_actor_and_partner_survive_restart_without_blocking_unrelated_indexing() {
    let app = init_app().await;
    let first = handler(&app);
    assert_eq!(first.start_block(STREAM, 1).await.unwrap(), 1);
    register(&app, "other-owner").await;
    let placement = placed(1, 1, 10, "LIC01-owner", Some("Partner-MiXeD"), true);
    first.handle_event(placement.clone()).await.unwrap();
    first
        .handle_event(event(
            2,
            OrderCancelledFilter::signature(),
            &[[1; 16]],
            vec![],
        ))
        .await
        .unwrap();
    first
        .handle_event(placed(3, 2, 20, "other-owner", None, false))
        .await
        .unwrap();
    first.finish_block(STREAM, 903).await.unwrap();
    first.checkpoint(STREAM, 4).await.unwrap();
    assert_eq!(pending(&app).await, 2);
    assert!(app
        .db_wrapper
        .orders()
        .get_order_by_id(&bytes16_to_hex([1; 16]).into())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        app.db_wrapper
            .orders()
            .filter_orders(Some(bytes16_to_hex([20; 16])), None, None)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(app
        .db_wrapper
        .orders()
        .filter_orders(Some(bytes16_to_hex([10; 16])), None, None)
        .await
        .is_err());
    assert!(app.db_wrapper.trades().get_all_trades().await.is_err());
    let response = reqwest::get(format!("{}/orders", app.address))
        .await
        .unwrap();
    assert_eq!(response.status(), 500);
    let gateway = crate::ewds_handler::mock_gateway().await;
    gsy_offchain_storage::ewds_handler::handle_request(
        &app.db_wrapper,
        &reqwest::Client::new(),
        &crate::ewds_handler::test_config(gateway.uri()),
        primitives::ewds::dto::EwdsRequestEnvelope {
            request_id: "pending-book".into(),
            operation: primitives::ewds::EwdsOperation::OrdersQuery,
            payload: serde_json::json!({"marketId": bytes16_to_hex([10; 16])}),
        },
    )
    .await
    .unwrap();
    let requests = gateway.received_requests().await.unwrap();
    let sent: primitives::ewds::dto::EwdsSendMessageDto =
        serde_json::from_slice(&requests[0].body).unwrap();
    let reply: serde_json::Value = serde_json::from_str(&sent.payload).unwrap();
    assert_eq!(reply["requestId"], "pending-book");
    assert_eq!(reply["success"], false);
    assert_eq!(reply["error"]["code"], "STORAGE_UNAVAILABLE");
    let stored = app
        .db_wrapper
        .collection::<Document>("evm_pending_events")
        .find_one(doc! {"block_number": 1})
        .await
        .unwrap()
        .unwrap();
    let original: ChainEvent =
        mongodb::bson::from_bson(stored.get("event").unwrap().clone()).unwrap();
    assert_eq!(original.log, placement.log);
    assert_eq!(original.block_timestamp, 901);

    // A new handler resumes the checkpoint but also retries older durable events.
    let restarted = handler(&app);
    assert_eq!(restarted.start_block(STREAM, 100).await.unwrap(), 4);
    register(&app, "LIC01-owner").await;
    restarted.finish_block(STREAM, 904).await.unwrap();
    assert_eq!(pending(&app).await, 2); // The preferred partner is still unresolved.
    register(&app, "Partner-MiXeD").await;
    restarted.finish_block(STREAM, 905).await.unwrap();
    assert_eq!(pending(&app).await, 0);
    let order = app
        .db_wrapper
        .orders()
        .get_order_by_id(&bytes16_to_hex([1; 16]).into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(order.created_by, "LIC01-owner");
    assert_eq!(order.area_uuid, "LIC01-owner");
    assert_eq!(
        order.requirements.unwrap().trading_partner_id.as_deref(),
        Some("Partner-MiXeD")
    );
    assert_eq!(order.status, OrderStatus::Cancelled);
    assert_eq!(order.status_updated_at, Some(902));
    // Replaying placement must not reset cancellation or duplicate the record.
    restarted.handle_event(placement).await.unwrap();
    restarted.finish_block(STREAM, 906).await.unwrap();
    let orders = app.db_wrapper.orders().get_all_orders().await.unwrap();
    assert_eq!(orders.len(), 2);
    assert_eq!(
        orders
            .iter()
            .find(|o| o.created_by == "LIC01-owner")
            .unwrap()
            .status,
        OrderStatus::Cancelled
    );
    stop_app(app).await;
}

#[tokio::test]
async fn deferred_match_penalty_and_verification_keep_chain_order_and_timestamps() {
    let app = init_app().await;
    let handler = handler(&app);
    register(&app, "buyer").await;
    register(&app, "seller").await;
    for placement in [
        placed(1, 1, 10, "buyer", Some("late-partner"), true),
        placed(2, 2, 10, "seller", None, false),
    ] {
        handler.handle_event(placement).await.unwrap();
    }
    handler
        .handle_event(event(
            3,
            TradeSettledFilter::signature(),
            &[[3; 16], [1; 16], [2; 16]],
            vec![
                bytes(hash("buyer")),
                bytes(hash("seller")),
                bytes([10; 16]),
                uint(1000),
                bytes([0; 16]),
                bytes([0; 16]),
                uint(10_000),
                uint(50_000),
            ],
        ))
        .await
        .unwrap();
    handler
        .handle_event(event(
            4,
            PenaltyRecordedFilter::signature(),
            &[hash("seller"), [10; 16], [3; 16]],
            vec![uint(100)],
        ))
        .await
        .unwrap();
    handler
        .handle_event(event(
            5,
            DeliveryVerifiedFilter::signature(),
            &[[3; 16], [1; 16], [2; 16]],
            vec![],
        ))
        .await
        .unwrap();
    assert_eq!(pending(&app).await, 5);
    handler.finish_block(STREAM, 905).await.unwrap();
    assert_eq!(pending(&app).await, 5);
    assert_eq!(
        app.db_wrapper
            .trades()
            .count_documents(doc! {})
            .await
            .unwrap(),
        0
    );
    register(&app, "late-partner").await;
    handler.finish_block(STREAM, 2000).await.unwrap();
    assert_eq!(pending(&app).await, 0);
    let trade = app
        .db_wrapper
        .trades()
        .get_all_trades()
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        (trade.buyer.as_str(), trade.seller.as_str()),
        ("buyer", "seller")
    );
    assert_eq!(trade.status, TradeStatus::Penalized);
    assert_eq!(trade.creation_time, 903);
    assert_eq!(trade.status_updated_at, Some(904));
    for order in app.db_wrapper.orders().get_all_orders().await.unwrap() {
        assert_eq!(order.status, OrderStatus::Executed);
        assert_eq!(order.status_updated_at, Some(905));
    }
    stop_app(app).await;
}

#[tokio::test]
async fn persistence_errors_are_not_misclassified_as_missing_mappings() {
    let app = init_app().await;
    let handler = handler(&app);
    register(&app, "owner").await;
    handler.start_block(STREAM, 1).await.unwrap();
    app.db_wrapper
        .run_command(doc! {"collMod": "orders", "validator": {"$expr": false}})
        .await
        .unwrap();
    handler
        .handle_event(placed(1, 1, 10, "owner", None, true))
        .await
        .unwrap();
    assert!(handler.finish_block(STREAM, 901).await.is_err());
    assert_eq!(pending(&app).await, 1);
    assert_eq!(handler.start_block(STREAM, 100).await.unwrap(), 1);
    app.db_wrapper
        .run_command(doc! {"collMod": "orders", "validator": {}})
        .await
        .unwrap();
    handler.finish_block(STREAM, 902).await.unwrap();
    assert_eq!(pending(&app).await, 0);
    assert_eq!(
        app.db_wrapper
            .orders()
            .get_all_orders()
            .await
            .unwrap()
            .len(),
        1
    );
    stop_app(app).await;
}

#[tokio::test]
async fn unresolved_market_mapping_defers_its_orders_and_expiry_only() {
    use gsy_ethers_listener::NewMarketCreatedFilter;
    let app = init_app().await;
    let handler = handler(&app);
    register(&app, "owner").await;
    handler
        .handle_event(event(
            1,
            NewMarketCreatedFilter::signature(),
            &[[10; 16], hash("Community-MiXeD")],
            vec![
                uint(800),
                uint(910),
                uint(1000),
                uint(1900),
                uint(0),
                uint(0),
                uint(901),
            ],
        ))
        .await
        .unwrap();
    handler
        .handle_event(placed(2, 1, 10, "owner", None, true))
        .await
        .unwrap();
    handler.finish_block(STREAM, 920).await.unwrap();
    assert_eq!(pending(&app).await, 2);
    // UUID spelling cannot bypass the pending projection guard.
    assert!(app
        .db_wrapper
        .markets()
        .filter(
            Some(uuid::Uuid::from_bytes([10; 16]).to_string()),
            None,
            None,
            None
        )
        .await
        .is_err());
    let response = reqwest::Client::new()
        .post(format!("{}/facilities/validate", app.address))
        .json(&serde_json::json!([{
            "facility_id": "facility",
            "market_id": uuid::Uuid::from_bytes([10; 16]).to_string()
        }]))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 500); // Retry, not a definitive unknown_market rejection.
    assert!(app
        .db_wrapper
        .orders()
        .get_order_by_id(&bytes16_to_hex([1; 16]).into())
        .await
        .unwrap()
        .is_none());
    register(&app, "Community-MiXeD").await;
    handler.finish_block(STREAM, 921).await.unwrap();
    assert_eq!(pending(&app).await, 0);
    let market = app
        .db_wrapper
        .markets()
        .filter(None, None, None, None)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(market.community_id, "Community-MiXeD");
    let order = app
        .db_wrapper
        .orders()
        .get_all_orders()
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(order.created_by, "owner");
    assert_eq!(order.status, OrderStatus::Expired);
    assert_eq!(order.status_updated_at, Some(910));
    stop_app(app).await;
}
