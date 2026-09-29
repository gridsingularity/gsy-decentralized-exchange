use crate::ewds_handler::{mock_gateway, test_config};
use crate::helpers::{init_app, stop_app};
use ethers::contract::LogMeta;
use ethers::types::{Address, H256, U256, U64};
use gsy_ethers_listener::{
    GsyEventHandler, MarketClearingFilter, MarketStatusUpdatedFilter, TradeSettledFilter,
};
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use gsy_offchain_storage::ewds_event_handler::EwdsEventPublisher;
use gsy_offchain_storage::ewds_handler::EwdsHandlerConfig;
use primitives::db_api_schema::trades::{
    ClearingResultSchema, ClearingStatus, DbTradeSchema, TradeParameters, TradeStatus,
};
use primitives::ewds::dto::{EwdsMarketStatusDto, EwdsSendMessageDto};
use primitives::ewds::EwdsEventType;
use primitives::utils::bytes16_to_hex;
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// --- Test helpers ---------------------------------------------------

/// Waits until the gateway received `expected` messages. The EVM handler does not expose the
/// publish task, so its events can only be observed at the gateway.
async fn wait_for_gateway_messages(
    server: &MockServer,
    expected: usize,
) -> Vec<EwdsSendMessageDto> {
    for _ in 0..100 {
        let requests = server.received_requests().await.unwrap();
        if requests.len() >= expected {
            return requests
                .iter()
                .map(|request| serde_json::from_slice(&request.body).unwrap())
                .collect();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("gateway did not receive {} message(s) in time", expected);
}

/// Gives a spawned publish task time to run, then asserts the gateway received nothing more.
async fn assert_no_further_gateway_messages(server: &MockServer, expected: usize) {
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.received_requests().await.unwrap().len(), expected);
}

fn event_payload(message: &EwdsSendMessageDto) -> serde_json::Value {
    serde_json::from_str(&message.payload).unwrap()
}

fn evm_handler(app: &crate::helpers::TestApp, server: &MockServer) -> OffchainStorageEvmHandler {
    OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: Some(EwdsEventPublisher::new(test_config(server.uri()))),
    }
}

fn trade_settled_event() -> TradeSettledFilter {
    TradeSettledFilter {
        trade_id: [0x01; 16],
        bid_id: [0x02; 16],
        offer_id: [0x03; 16],
        buyer_id: [0x04; 16],
        seller_id: [0x05; 16],
        market_id: [0x06; 16],
        time_slot: 1_000,
        residual_bid_id: [0; 16],
        residual_offer_id: [0; 16],
        energy: U256::from(20_000u64),
        price: U256::from(3_000u64),
    }
}

fn market_clearing_event(clearing_status: u8) -> MarketClearingFilter {
    MarketClearingFilter {
        market_id: [0x07; 16],
        clearing_status,
        clearing_price: U256::from(3_000u64),
        total_supply: U256::from(50_000u64),
        total_demand: U256::from(40_000u64),
        traded_quantity: U256::from(40_000u64),
        num_trades: 2,
    }
}

fn log_meta(transaction_hash: H256) -> LogMeta {
    LogMeta {
        address: Address::zero(),
        block_number: U64::from(1u64),
        block_hash: H256::zero(),
        transaction_hash,
        transaction_index: U64::zero(),
        log_index: U256::zero(),
    }
}

#[tokio::test]
async fn publish_trade_created_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));
    let trade = DbTradeSchema {
        trade_uuid: "trade-1".to_string(),
        status: TradeStatus::Settled,
        seller: "seller-1".to_string(),
        buyer: "buyer-1".to_string(),
        market_id: "market-1".to_string(),
        time_slot: 1_000,
        creation_time: 950,
        offer_hash: "offer-1".to_string(),
        bid_hash: "bid-1".to_string(),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 2.0,
            energy_rate: 0.3,
        },
    };

    publisher.publish_trade_created(trade).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "tradeCreated");
    assert_eq!(send_dto.transaction_id, "trade-created-trade-1");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_eq!(event["eventId"], json!("trade-created-trade-1"));
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::TradeCreated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(950));
    assert_eq!(event["data"]["tradeId"], json!("trade-1"));
    assert_eq!(event["data"]["marketId"], json!("market-1"));
}

#[tokio::test]
async fn publish_clearing_result_created_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));
    let clearing_result = ClearingResultSchema {
        market_id: "market-1".to_string(),
        clearing_status: ClearingStatus::Final,
        no_bid_reason: None,
        clearing_price: 0.3,
        total_supply: 5.0,
        total_demand: 4.0,
        traded_quantity: 4.0,
        num_trades: 2,
        tx_hash: "0xabc".to_string(),
        clearing_time: 1_100,
    };

    publisher
        .publish_clearing_result_created(clearing_result)
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "clearingResultCreated");
    assert_eq!(
        send_dto.transaction_id,
        "clearing-result-created-market-1-0xabc"
    );

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_eq!(
        event["eventId"],
        json!("clearing-result-created-market-1-0xabc")
    );
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::ClearingResultCreated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(1_100));
    assert_eq!(event["data"]["marketId"], json!("market-1"));
    assert_eq!(event["data"]["numTrades"], json!(2));
    assert_eq!(event["data"]["txHash"], json!("0xabc"));
}

#[tokio::test]
async fn publish_market_status_updated_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_market_status_updated(
            EwdsMarketStatusDto {
                market_id: "market-1".to_string(),
                is_open: false,
            },
            1_200,
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "marketStatusUpdated");
    assert_eq!(
        send_dto.transaction_id,
        "market-status-updated-market-1-closed"
    );

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_eq!(
        event["eventId"],
        json!("market-status-updated-market-1-closed")
    );
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::MarketStatusUpdated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(1_200));
    assert_eq!(
        event["data"],
        json!({"marketId": "market-1", "isOpen": false})
    );
}

#[tokio::test]
async fn publish_does_not_retry_or_panic_when_gateway_rejects_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("invalid topic"))
        .mount(&server)
        .await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    // The send error is logged inside the task; the task itself must still finish cleanly.
    publisher
        .publish_market_status_updated(
            EwdsMarketStatusDto {
                market_id: "market-1".to_string(),
                is_open: true,
            },
            1_200,
        )
        .await
        .expect("publish task should not panic on gateway errors");

    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[test]
fn handler_config_defaults_to_the_events_channels() {
    let config = EwdsHandlerConfig::from_env();
    for (env_key, actual, expected) in [
        (
            "EWDS_EVENT_PUBLISH_FQCN",
            config.event_publish_fqcn.as_str(),
            "gsy.intelligent.events.pub",
        ),
        (
            "EWDS_EVENT_SUBSCRIBE_FQCN",
            config.event_subscribe_fqcn.as_str(),
            "gsy.intelligent.events.sub",
        ),
        (
            "EWDS_TRADE_CREATED_EVENT_TOPIC",
            config.trade_created_topic.as_str(),
            "tradeCreated",
        ),
        (
            "EWDS_CLEARING_RESULT_CREATED_EVENT_TOPIC",
            config.clearing_result_created_topic.as_str(),
            "clearingResultCreated",
        ),
        (
            "EWDS_MARKET_STATUS_UPDATED_EVENT_TOPIC",
            config.market_status_updated_topic.as_str(),
            "marketStatusUpdated",
        ),
    ] {
        // Only the defaults are under test; an explicitly configured value is left alone.
        if std::env::var(env_key).is_err() {
            assert_eq!(actual, expected, "unexpected default for {}", env_key);
        }
    }
}

// --- EVM handler integration ----------------------------------------

#[tokio::test]
async fn trade_settled_publishes_trade_created_event() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);

    handler
        .handle_trade_settled(trade_settled_event())
        .await
        .unwrap();

    let messages = wait_for_gateway_messages(&server, 1).await;
    let trade_id = bytes16_to_hex([0x01; 16]);
    assert_eq!(messages[0].topic_name, "tradeCreated");
    let event = event_payload(&messages[0]);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::TradeCreated.as_str())
    );
    assert_eq!(
        event["eventId"],
        json!(format!("trade-created-{}", trade_id))
    );
    assert_eq!(event["data"]["tradeId"], json!(trade_id));
    assert_eq!(event["data"]["bidId"], json!(bytes16_to_hex([0x02; 16])));
    assert_eq!(event["data"]["offerId"], json!(bytes16_to_hex([0x03; 16])));
    assert_eq!(event["data"]["tradeStatus"], json!("settled"));
    assert_eq!(event["data"]["residualBidId"], json!(null));

    let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(event["occurredAt"], json!(stored[0].creation_time));

    stop_app(app).await;
}

#[tokio::test]
async fn trade_settled_does_not_publish_when_trade_is_not_persisted() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);

    handler
        .handle_trade_settled(trade_settled_event())
        .await
        .unwrap();
    wait_for_gateway_messages(&server, 1).await;

    // The unique trade_uuid index rejects the duplicate, so no second event may be sent.
    assert!(handler
        .handle_trade_settled(trade_settled_event())
        .await
        .is_err());
    assert_no_further_gateway_messages(&server, 1).await;

    stop_app(app).await;
}

#[tokio::test]
async fn market_clearing_publishes_clearing_result_created_event() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    let transaction_hash = H256::repeat_byte(0xab);

    handler
        .handle_market_clearing(market_clearing_event(1), log_meta(transaction_hash), 1_100)
        .await
        .unwrap();

    let messages = wait_for_gateway_messages(&server, 1).await;
    let market_id = bytes16_to_hex([0x07; 16]);
    let tx_hash = format!("{:?}", transaction_hash);
    assert_eq!(messages[0].topic_name, "clearingResultCreated");
    let event = event_payload(&messages[0]);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::ClearingResultCreated.as_str())
    );
    assert_eq!(
        event["eventId"],
        json!(format!("clearing-result-created-{}-{}", market_id, tx_hash))
    );
    assert_eq!(event["occurredAt"], json!(1_100));
    assert_eq!(event["data"]["marketId"], json!(market_id));
    assert_eq!(event["data"]["clearingStatus"], json!("final"));
    assert_eq!(event["data"]["numTrades"], json!(2));
    assert_eq!(event["data"]["txHash"], json!(tx_hash));

    stop_app(app).await;
}

#[tokio::test]
async fn market_clearing_does_not_publish_for_invalid_clearing_status() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);

    assert!(handler
        .handle_market_clearing(market_clearing_event(99), log_meta(H256::zero()), 1_100)
        .await
        .is_err());
    assert_no_further_gateway_messages(&server, 0).await;

    stop_app(app).await;
}

#[tokio::test]
async fn market_status_publishes_market_status_updated_event() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    let before = chrono::Utc::now().timestamp() as u64;

    handler
        .handle_market_status(MarketStatusUpdatedFilter {
            market_id: [0x08; 16],
            is_open: true,
        })
        .await
        .unwrap();

    let messages = wait_for_gateway_messages(&server, 1).await;
    let market_id = bytes16_to_hex([0x08; 16]);
    assert_eq!(messages[0].topic_name, "marketStatusUpdated");
    let event = event_payload(&messages[0]);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::MarketStatusUpdated.as_str())
    );
    assert_eq!(
        event["eventId"],
        json!(format!("market-status-updated-{}-open", market_id))
    );
    assert_eq!(
        event["data"],
        json!({"marketId": market_id, "isOpen": true})
    );
    // Market status is not persisted, so occurredAt is the time the event was received.
    let occurred_at = event["occurredAt"].as_u64().unwrap();
    assert!(occurred_at >= before && occurred_at <= chrono::Utc::now().timestamp() as u64);

    stop_app(app).await;
}

#[test]
fn handler_config_maps_event_types_to_their_topics() {
    let config = test_config("http://gateway".to_string());
    for (event_type, topic) in [
        (EwdsEventType::TradeCreated, "tradeCreated"),
        (
            EwdsEventType::ClearingResultCreated,
            "clearingResultCreated",
        ),
        (EwdsEventType::MarketStatusUpdated, "marketStatusUpdated"),
    ] {
        assert_eq!(config.event_topic(event_type), topic);
    }
}
