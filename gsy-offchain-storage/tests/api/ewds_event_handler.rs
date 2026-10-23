use crate::evm_integration::{expected_market, new_market_event};
use crate::ewds_handler::{mock_gateway, test_config};
use crate::helpers::{init_app, stop_app};
use ethers::contract::LogMeta;
use ethers::types::{Address, H256, U256, U64};
use gsy_ethers_listener::{
    DeliveryVerifiedFilter, GsyEventHandler, MarketClearingFilter, NewMarketCreatedFilter,
    OrderCancelledFilter, OrderPlacedFilter, PenaltyRecordedFilter, TradeSettledFilter,
};
use gsy_offchain_storage::db::DatabaseWrapper;
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use gsy_offchain_storage::ewds_event_handler::{
    handle_event, start_ewds_event_subscriber, EwdsEventPublisher,
};
use gsy_offchain_storage::ewds_handler::{handle_request, EwdsHandlerConfig};
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema};
use primitives::db_api_schema::orders::OrderStatus;
use primitives::db_api_schema::profiles::MeasurementPointType;
use primitives::db_api_schema::trades::{
    ClearingResultSchema, ClearingStatus, DbTradeSchema, TradeParameters, TradeStatus,
};
use primitives::ewds::dto::{
    EwdsEventEnvelope, EwdsMarketDto, EwdsOrderDto, EwdsRequestEnvelope, EwdsSendMessageDto,
    EwdsTradeDto,
};
use primitives::ewds::{is_invalid_event, EwdsEventType, EwdsOperation};
use primitives::utils::{bytes16_to_hex, epoch_to_rfc3339, timestamp_to_string_with_padding};
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{method, path, query_param};
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

/// Every event gets a random UUID as its ID, which is also its DDHub transaction ID.
fn assert_event_id(message: &EwdsSendMessageDto, event: &serde_json::Value) {
    assert!(
        Uuid::parse_str(&message.transaction_id).is_ok(),
        "{}",
        message.transaction_id
    );
    assert_eq!(event["eventId"], json!(message.transaction_id));
}

fn event_payload(message: &EwdsSendMessageDto) -> serde_json::Value {
    serde_json::from_str(&message.payload).unwrap()
}

/// The `market.event` item for the market of `new_market_event()`.
fn market_event_dto() -> EwdsMarketDto {
    EwdsMarketDto::try_from(expected_market()).unwrap()
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

fn order_placed_event(order_id: [u8; 16], is_bid: bool) -> OrderPlacedFilter {
    OrderPlacedFilter {
        order_id,
        created_by: if is_bid { [0x04; 16] } else { [0x05; 16] },
        market_id: [0x06; 16],
        time_slot: 1_000,
        creation_time: 900,
        energy: 20_000,
        energy_rate: 3_000,
        energy_source_preference: 0,
        energy_type: 0,
        is_bid,
        preferred_trading_partner: [0; 16],
        preferred_energy_rate: 0,
    }
}

async fn seed_settlement_orders(app: &crate::helpers::TestApp) {
    let handler = OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    };
    for (id, is_bid) in [([0x02; 16], true), ([0x03; 16], false)] {
        handler
            .handle_order_placed(order_placed_event(id, is_bid), 950)
            .await
            .unwrap();
    }
}

async fn assert_trade_status_event(
    app: &crate::helpers::TestApp,
    messages: &[EwdsSendMessageDto],
    status: TradeStatus,
    timestamp: u64,
) {
    let message = messages
        .iter()
        .find(|message| message.topic_name == "trade")
        .unwrap();
    let envelope = event_payload(message);
    assert_eq!(message.fqcn, "gsy.events.pub");
    assert_eq!(envelope["eventType"], EwdsEventType::Trade.as_str());
    assert_eq!(envelope["occurredAt"], epoch_to_rfc3339(timestamp));
    assert_event_id(message, &envelope);
    let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, status);
    assert_eq!(stored[0].status_updated_at, Some(timestamp));
    let expected = json!([EwdsTradeDto::from(stored[0].clone())]);
    assert_eq!(envelope["data"], expected);
    let client = reqwest::Client::new();
    let http = client
        .get(format!("{}/trades", app.address))
        .query(&[("market_id", &stored[0].market_id)])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(http, expected);
    let server = mock_gateway().await;
    handle_request(
        &app.db_wrapper,
        &client,
        &test_config(server.uri()),
        EwdsRequestEnvelope {
            request_id: "status-query".into(),
            operation: EwdsOperation::TradesQuery,
            payload: json!({"marketId": stored[0].market_id}),
        },
    )
    .await
    .unwrap();
    let replies = wait_for_gateway_messages(&server, 1).await;
    assert_eq!(event_payload(&replies[0])["data"], expected);
}

async fn assert_order_status_event(
    app: &crate::helpers::TestApp,
    messages: &[EwdsSendMessageDto],
    expected: &[([u8; 16], OrderStatus)],
    timestamp: u64,
) {
    let message = messages
        .iter()
        .find(|message| event_payload(message)["eventType"] == EwdsEventType::OrderStatus.as_str())
        .expect("missing order-status event");
    assert_eq!(message.topic_name, "order");
    assert_eq!(message.fqcn, "gsy.events.pub");
    let envelope = event_payload(message);
    assert_event_id(message, &envelope);
    assert_eq!(envelope["occurredAt"], epoch_to_rfc3339(timestamp));
    let data = envelope["data"].as_array().unwrap();
    assert_eq!(data.len(), expected.len());
    for (id, status) in expected {
        let id = bytes16_to_hex(*id);
        let stored = app
            .db_wrapper
            .orders()
            .get_order_by_id(&id.clone().into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&stored.status, status);
        assert_eq!(stored.status_updated_at, Some(timestamp));
        let snapshot = data.iter().find(|order| order["orderId"] == id).unwrap();
        assert_eq!(
            *snapshot,
            serde_json::to_value(EwdsOrderDto::from(stored)).unwrap()
        );
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

fn trade(trade_uuid: &str, creation_time: u64) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: trade_uuid.to_string(),
        status: TradeStatus::Matched,
        seller: "seller-1".to_string(),
        buyer: "buyer-1".to_string(),
        market_id: "market-1".to_string(),
        creation_time,
        offer_hash: "offer-1".to_string(),
        bid_hash: "bid-1".to_string(),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 2.0,
            energy_rate: 0.3,
        },
        status_updated_at: None,
    }
}

#[tokio::test]
async fn publish_trade_event_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_trade_event(vec![trade("trade-1", 950)])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "trade");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(event["eventType"], json!(EwdsEventType::Trade.as_str()));
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(950)));
    assert_eq!(event["data"].as_array().unwrap().len(), 1);
    assert_eq!(event["data"][0]["tradeId"], json!("trade-1"));
    assert_eq!(event["data"][0]["marketId"], json!("market-1"));
}

#[tokio::test]
async fn publish_trade_event_sends_several_trades_in_one_event() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    let mut updated = trade("trade-1", 950);
    updated.status_updated_at = Some(1_100);
    publisher
        .publish_trade_event(vec![updated, trade("trade-2", 960)])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    // Use the latest status time, falling back to creation for legacy records.
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_100)));
    assert_eq!(event["data"][0]["timestamp"], epoch_to_rfc3339(950));
    assert_eq!(event["data"][0]["tradeId"], json!("trade-1"));
    assert_eq!(event["data"][1]["tradeId"], json!("trade-2"));
}

#[tokio::test]
async fn verification_notifications_preserve_penalized_status_and_match_query_responses() {
    for penalized in [false, true] {
        let app = init_app().await;
        let server = mock_gateway().await;
        let handler = evm_handler(&app, &server);
        seed_settlement_orders(&app).await;
        let settlement = trade_settled_event();
        handler
            .handle_trade_settled(settlement.clone(), 1_100)
            .await
            .unwrap();
        let messages = wait_for_gateway_messages(&server, 2).await;
        assert_trade_status_event(&app, &messages, TradeStatus::Matched, 1_100).await;
        let mut count = 2;
        let mut penalty = PenaltyRecordedFilter {
            trade_id: settlement.trade_id,
            market_id: settlement.market_id,
            penalized_actor_id: [0xff; 16],
            penalty_energy: 10,
        };
        assert!(handler
            .handle_penalty_recorded(penalty.clone(), 1_200)
            .await
            .is_err());
        if penalized {
            penalty.penalized_actor_id = settlement.buyer_id;
            for timestamp in [1_200, 1_201] {
                handler
                    .handle_penalty_recorded(penalty.clone(), timestamp)
                    .await
                    .unwrap();
                let messages = wait_for_gateway_messages(&server, count + 1).await;
                assert_trade_status_event(&app, &messages[count..], TradeStatus::Penalized, 1_200)
                    .await;
                count += 1;
            }
        }
        for timestamp in [1_250, 1_300] {
            handler
                .handle_delivery_verified(
                    DeliveryVerifiedFilter {
                        trade_id: settlement.trade_id,
                        bid_id: settlement.bid_id,
                        offer_id: settlement.offer_id,
                    },
                    timestamp,
                )
                .await
                .unwrap();
            let messages = wait_for_gateway_messages(&server, count + 2).await;
            assert_trade_status_event(
                &app,
                &messages[count..],
                if penalized {
                    TradeStatus::Penalized
                } else {
                    TradeStatus::Executed
                },
                if penalized { 1_200 } else { 1_250 },
            )
            .await;
            assert_order_status_event(
                &app,
                &messages[count..],
                &[
                    (settlement.bid_id, OrderStatus::Executed),
                    (settlement.offer_id, OrderStatus::Executed),
                ],
                1_250,
            )
            .await;
            count += 2;
        }
        assert_no_further_gateway_messages(&server, count).await;
        stop_app(app).await;
    }
}

#[tokio::test]
async fn every_event_gets_its_own_id() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    for _ in 0..2 {
        publisher
            .publish_trade_event(vec![trade("trade-1", 950)])
            .await
            .unwrap();
    }

    let requests = server.received_requests().await.unwrap();
    let ids = requests
        .iter()
        .map(|request| {
            serde_json::from_slice::<EwdsSendMessageDto>(&request.body)
                .unwrap()
                .transaction_id
        })
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn publish_clearing_result_event_sends_event_on_events_channel() {
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
        .publish_clearing_result_event(vec![clearing_result])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "clearingResult");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::ClearingResult.as_str())
    );
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_100)));
    assert_eq!(event["data"][0]["marketId"], json!("market-1"));
    assert_eq!(event["data"][0]["numTrades"], json!(2));
    assert_eq!(event["data"][0]["txHash"], json!("0xabc"));
}

#[tokio::test]
async fn publish_market_event_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_market_event(vec![market_event_dto()], 1_200)
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "market");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(event["eventType"], json!(EwdsEventType::Market.as_str()));
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_200)));
    assert_eq!(
        event["data"],
        json!([{
            "marketId": "cccccccc-cccc-cccc-cccc-cccccccccccc",
            "communityId": "11111111-1111-4111-8111-111111111111",
            "openingTime": epoch_to_rfc3339(1_699_998_300),
            "closingTime": epoch_to_rfc3339(1_700_000_100),
            "deliveryStartTime": epoch_to_rfc3339(1_700_000_100),
            "deliveryEndTime": epoch_to_rfc3339(1_700_001_000),
            "marketType": "flex",
            "matchingAlgorithm": "pay_as_clear",
            "createdAt": epoch_to_rfc3339(1_699_998_301),
        }])
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
        .publish_market_event(vec![market_event_dto()], 1_200)
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
            "EWDS_TRADE_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Trade),
            "trade",
        ),
        (
            "EWDS_CLEARING_RESULT_EVENT_TOPIC",
            config.event_topic(EwdsEventType::ClearingResult),
            "clearingResult",
        ),
        (
            "EWDS_MARKET_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Market),
            "market",
        ),
        (
            "EWDS_MEASUREMENTS_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Measurements),
            "measurements",
        ),
        (
            "EWDS_FACILITY_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Facility),
            "facility",
        ),
        (
            "EWDS_SITE_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Site),
            "site",
        ),
        (
            "EWDS_COMMUNITY_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Community),
            "community",
        ),
        (
            "EWDS_ORDER_EVENT_TOPIC",
            config.event_topic(EwdsEventType::Order),
            "order",
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
async fn trade_settled_publishes_trade_event() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    seed_settlement_orders(&app).await;

    handler
        .handle_trade_settled(trade_settled_event(), 1_100)
        .await
        .unwrap();

    let messages = wait_for_gateway_messages(&server, 2).await;
    assert_order_status_event(
        &app,
        &messages,
        &[
            ([0x02; 16], OrderStatus::Filled),
            ([0x03; 16], OrderStatus::Filled),
        ],
        1_100,
    )
    .await;
    let message = messages
        .iter()
        .find(|message| message.topic_name == "trade")
        .unwrap();
    let trade_id = bytes16_to_hex([0x01; 16]);
    let event = event_payload(message);
    assert_eq!(event["eventType"], json!(EwdsEventType::Trade.as_str()));
    assert_event_id(message, &event);
    assert_eq!(event["data"].as_array().unwrap().len(), 1);
    assert_eq!(event["data"][0]["tradeId"], json!(trade_id));
    assert_eq!(event["data"][0]["bidId"], json!(bytes16_to_hex([0x02; 16])));
    assert_eq!(
        event["data"][0]["offerId"],
        json!(bytes16_to_hex([0x03; 16]))
    );
    assert_eq!(event["data"][0]["tradeStatus"], json!("matched"));
    assert_eq!(event["data"][0]["residualBidId"], json!(null));

    let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(
        event["occurredAt"],
        json!(epoch_to_rfc3339(stored[0].creation_time))
    );

    stop_app(app).await;
}

#[tokio::test]
async fn trade_settled_rejects_conflicting_details_without_publishing() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    seed_settlement_orders(&app).await;

    handler
        .handle_trade_settled(trade_settled_event(), 1_100)
        .await
        .unwrap();
    wait_for_gateway_messages(&server, 2).await;

    let before =
        serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap();
    let orders_before = app.db_wrapper.orders().get_all_orders().await.unwrap();
    for field in [
        "buyer",
        "seller",
        "market",
        "bid",
        "offer",
        "residual_bid",
        "residual_offer",
        "energy",
        "price",
    ] {
        let mut event = trade_settled_event();
        match field {
            "buyer" => event.buyer_id = [0xff; 16],
            "seller" => event.seller_id = [0xff; 16],
            "market" => event.market_id = [0xff; 16],
            "bid" => event.bid_id = [0xff; 16],
            "offer" => event.offer_id = [0xff; 16],
            "residual_bid" => event.residual_bid_id = [0xff; 16],
            "residual_offer" => event.residual_offer_id = [0xff; 16],
            "energy" => event.energy += U256::one(),
            "price" => event.price += U256::one(),
            _ => unreachable!(),
        }
        let error = handler
            .handle_trade_settled(event, 1_101)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("Failed to index trade"),
            "{field}: {error:?}"
        );
    }
    assert_eq!(
        serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap(),
        before
    );
    assert_eq!(
        app.db_wrapper.orders().get_all_orders().await.unwrap(),
        orders_before
    );
    assert_no_further_gateway_messages(&server, 2).await;

    stop_app(app).await;
}

#[tokio::test]
async fn trade_settled_preserves_replayed_statuses() {
    for (status, updated_at) in [
        (TradeStatus::Matched, Some(960)),
        (TradeStatus::Matched, None),
        (TradeStatus::Settled, Some(1_100)),
        (TradeStatus::Settled, None),
        (TradeStatus::Executed, Some(1_200)),
        (TradeStatus::Penalized, Some(1_200)),
    ] {
        let app = init_app().await;
        let server = mock_gateway().await;
        let handler = evm_handler(&app, &server);
        seed_settlement_orders(&app).await;
        let event = trade_settled_event();
        let expected = DbTradeSchema {
            status: status.clone(),
            status_updated_at: updated_at,
            buyer: bytes16_to_hex(event.buyer_id),
            seller: bytes16_to_hex(event.seller_id),
            market_id: bytes16_to_hex(event.market_id),
            bid_hash: bytes16_to_hex(event.bid_id),
            offer_hash: bytes16_to_hex(event.offer_id),
            ..trade(&bytes16_to_hex(event.trade_id), 960)
        };
        app.db_wrapper
            .trades()
            .insert_one(expected.clone())
            .await
            .unwrap();
        let executed = matches!(status, TradeStatus::Executed | TradeStatus::Penalized);
        if executed {
            app.db_wrapper.orders().update_many(
                mongodb::bson::doc! {},
                mongodb::bson::doc! {"$set": {"status": "executed", "status_updated_at": 1_200i64}},
            ).await.unwrap();
        }
        for (index, timestamp) in [1_100, 1_300].into_iter().enumerate() {
            handler
                .handle_trade_settled(event.clone(), timestamp)
                .await
                .unwrap();
            let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
            assert_eq!(
                serde_json::to_value(stored).unwrap(),
                json!([expected.clone()])
            );
            let messages = wait_for_gateway_messages(&server, (index + 1) * 2).await;
            let new_messages = &messages[index * 2..];
            let order_status = if executed {
                OrderStatus::Executed
            } else {
                OrderStatus::Filled
            };
            assert_order_status_event(
                &app,
                new_messages,
                &[
                    (event.bid_id, order_status.clone()),
                    (event.offer_id, order_status),
                ],
                if executed { 1_200 } else { 1_100 },
            )
            .await;
            let message = new_messages
                .iter()
                .find(|message| message.topic_name == "trade")
                .unwrap();
            assert_eq!(
                event_payload(message)["data"],
                json!([EwdsTradeDto::from(expected.clone())])
            );
            assert_eq!(
                event_payload(message)["occurredAt"],
                epoch_to_rfc3339(expected.status_updated_at.unwrap_or(expected.creation_time))
            );
        }
        stop_app(app).await;
    }
}

#[tokio::test]
async fn trade_settled_replay_completes_a_partially_persisted_fill() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    seed_settlement_orders(&app).await;
    let event = trade_settled_event();
    // Simulate an order that has not been indexed yet: the trade and bid write succeed.
    app.db_wrapper
        .orders()
        .delete_one(mongodb::bson::doc! {"order_id": bytes16_to_hex(event.offer_id)})
        .await
        .unwrap();
    assert!(handler
        .handle_trade_settled(event.clone(), 1_100)
        .await
        .is_err());
    let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].status, TradeStatus::Matched);
    assert_eq!(trades[0].status_updated_at, Some(1_100));
    assert_no_further_gateway_messages(&server, 0).await;
    let quiet_handler = OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    };
    quiet_handler
        .handle_order_placed(order_placed_event(event.offer_id, false), 950)
        .await
        .unwrap();
    // Closure can be indexed before this delayed settlement is retried.
    app.db_wrapper
        .orders()
        .expire_market_orders(&bytes16_to_hex(event.market_id), 1_150)
        .await
        .unwrap();
    for (index, timestamp) in [1_100, 1_101].into_iter().enumerate() {
        handler
            .handle_trade_settled(event.clone(), timestamp)
            .await
            .unwrap();
        let messages = wait_for_gateway_messages(&server, (index + 1) * 2).await;
        assert_order_status_event(
            &app,
            &messages[index * 2..],
            &[
                (event.bid_id, OrderStatus::Filled),
                (event.offer_id, OrderStatus::Filled),
            ],
            1_100,
        )
        .await;
        assert_eq!(
            serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap(),
            json!(trades)
        );
    }
    stop_app(app).await;
}

#[tokio::test]
async fn market_clearing_publishes_clearing_result_event() {
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
    assert_eq!(messages[0].topic_name, "clearingResult");
    let event = event_payload(&messages[0]);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::ClearingResult.as_str())
    );
    assert_event_id(&messages[0], &event);
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_100)));
    assert_eq!(event["data"][0]["marketId"], json!(market_id));
    assert_eq!(event["data"][0]["clearingStatus"], json!("final"));
    assert_eq!(event["data"][0]["numTrades"], json!(2));
    assert_eq!(event["data"][0]["txHash"], json!(tx_hash));

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
async fn new_market_publishes_market_event() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);

    handler
        .handle_new_market_created(new_market_event())
        .await
        .unwrap();

    let messages = wait_for_gateway_messages(&server, 1).await;
    assert_eq!(messages[0].topic_name, "market");
    let event = event_payload(&messages[0]);
    assert_eq!(event["eventType"], json!(EwdsEventType::Market.as_str()));
    assert_event_id(&messages[0], &event);
    assert_eq!(event["data"], json!([market_event_dto()]));
    assert_eq!(
        event["occurredAt"],
        json!(epoch_to_rfc3339(new_market_event().created_at))
    );

    stop_app(app).await;
}

#[test]
fn handler_config_maps_event_types_to_their_topics() {
    let config = test_config("http://gateway".to_string());
    for (event_type, topic) in [
        (EwdsEventType::Trade, "trade"),
        (EwdsEventType::ClearingResult, "clearingResult"),
        (EwdsEventType::Market, "market"),
        (EwdsEventType::Measurements, "measurements"),
        (EwdsEventType::Facility, "facility"),
        (EwdsEventType::Site, "site"),
        (EwdsEventType::Community, "community"),
        (EwdsEventType::Order, "order"),
        (EwdsEventType::OrderStatus, "order"),
    ] {
        assert_eq!(config.event_topic(event_type), topic);
    }
}

#[tokio::test]
async fn order_status_notifications_follow_consumed_orders_and_independent_residuals() {
    for residual_is_bid in [true, false] {
        let app = init_app().await;
        let server = mock_gateway().await;
        let handler = evm_handler(&app, &server);
        seed_settlement_orders(&app).await;
        let parent_id = if residual_is_bid {
            [0x02; 16]
        } else {
            [0x03; 16]
        };
        let mut parent = order_placed_event(parent_id, residual_is_bid);
        parent.energy = 30_000;
        handler.handle_order_placed(parent, 960).await.unwrap();
        let messages = wait_for_gateway_messages(&server, 1).await;
        assert_order_status_event(&app, &messages, &[(parent_id, OrderStatus::Submitted)], 960)
            .await;

        let mut settled = trade_settled_event();
        if residual_is_bid {
            settled.residual_bid_id = [0x07; 16];
        } else {
            settled.residual_offer_id = [0x07; 16];
        }
        handler.handle_trade_settled(settled, 1_100).await.unwrap();
        let messages = wait_for_gateway_messages(&server, 3).await;
        let bid_status = if residual_is_bid {
            OrderStatus::PartiallyFilled
        } else {
            OrderStatus::Filled
        };
        let offer_status = if residual_is_bid {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };
        assert_order_status_event(
            &app,
            &messages[1..],
            &[([0x02; 16], bid_status), ([0x03; 16], offer_status)],
            1_100,
        )
        .await;

        let mut residual = order_placed_event([0x07; 16], residual_is_bid);
        residual.energy = 10_000;
        handler.handle_order_placed(residual, 1_100).await.unwrap();
        let messages = wait_for_gateway_messages(&server, 4).await;
        assert_order_status_event(
            &app,
            &messages[3..],
            &[([0x07; 16], OrderStatus::Submitted)],
            1_100,
        )
        .await;

        // A replay may notify again, but keeps the original persisted status timestamp.
        for (index, timestamp) in [1_200, 1_201].into_iter().enumerate() {
            handler
                .handle_delivery_verified(
                    DeliveryVerifiedFilter {
                        trade_id: [0x01; 16],
                        bid_id: [0x02; 16],
                        offer_id: [0x03; 16],
                    },
                    timestamp,
                )
                .await
                .unwrap();
            let messages = wait_for_gateway_messages(&server, 6 + index * 2).await;
            assert_order_status_event(
                &app,
                &messages[4 + index * 2..],
                &[
                    ([0x02; 16], OrderStatus::Executed),
                    ([0x03; 16], OrderStatus::Executed),
                ],
                1_200,
            )
            .await;
            let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
            assert_eq!(trades[0].status, TradeStatus::Executed);
            assert_eq!(trades[0].status_updated_at, Some(1_200));
            let residual = app
                .db_wrapper
                .orders()
                .get_order_by_id(&bytes16_to_hex([0x07; 16]).into())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(residual.status, OrderStatus::Submitted);
            assert_eq!(residual.energy_kWh, 1.0);
        }

        handler
            .handle_order_cancelled(
                OrderCancelledFilter {
                    order_id: [0x07; 16],
                },
                1_300,
            )
            .await
            .unwrap();
        let messages = wait_for_gateway_messages(&server, 9).await;
        assert_order_status_event(
            &app,
            &messages[8..],
            &[([0x07; 16], OrderStatus::Cancelled)],
            1_300,
        )
        .await;
        assert_no_further_gateway_messages(&server, 9).await;
        stop_app(app).await;
    }
}

#[tokio::test]
async fn market_closure_notifies_only_newly_expired_orders_in_that_market() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    let quiet_handler = OffchainStorageEvmHandler {
        db: app.db_wrapper.clone(),
        event_publisher: None,
    };
    let statuses = [
        OrderStatus::Submitted,
        OrderStatus::Submitted,
        OrderStatus::Filled,
        OrderStatus::PartiallyFilled,
        OrderStatus::Cancelled,
        OrderStatus::Expired,
        OrderStatus::Executed,
    ];
    for (index, status) in statuses.iter().enumerate() {
        let id = [index as u8 + 1; 16];
        let mut placed = order_placed_event(id, true);
        if index == 1 {
            placed.market_id = [0x09; 16];
        }
        quiet_handler
            .handle_order_placed(placed, 950)
            .await
            .unwrap();
        app.db_wrapper
            .orders()
            .update_order_status_by_id(&bytes16_to_hex(id).into(), status.clone(), 950)
            .await
            .unwrap();
    }

    quiet_handler
        .handle_new_market_created(NewMarketCreatedFilter {
            market_id: [0x06; 16],
            closing_time: 1_100,
            ..new_market_event()
        })
        .await
        .unwrap();
    handler.handle_block(1_099).await.unwrap();
    assert_no_further_gateway_messages(&server, 0).await;
    handler.handle_block(1_100).await.unwrap();
    let messages = wait_for_gateway_messages(&server, 1).await;
    assert_order_status_event(&app, &messages, &[([1; 16], OrderStatus::Expired)], 1_100).await;
    handler.handle_block(1_101).await.unwrap();
    assert_no_further_gateway_messages(&server, 1).await;
    for (index, status) in statuses.iter().enumerate() {
        let stored = app
            .db_wrapper
            .orders()
            .get_order_by_id(&bytes16_to_hex([index as u8 + 1; 16]).into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            &stored.status,
            if index == 0 {
                &OrderStatus::Expired
            } else {
                status
            }
        );
        assert_eq!(
            stored.status_updated_at,
            Some(if index == 0 { 1_100 } else { 950 })
        );
    }
    assert_no_further_gateway_messages(&server, 1).await;
    stop_app(app).await;
}

#[tokio::test]
async fn failed_order_indexing_does_not_publish_status_notifications() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let handler = evm_handler(&app, &server);
    let mut placed = order_placed_event([0x02; 16], true);
    placed.preferred_trading_partner = [0x99; 16];
    assert!(handler.handle_order_placed(placed, 950).await.is_err());
    assert!(handler
        .handle_order_cancelled(
            OrderCancelledFilter {
                order_id: [0x02; 16]
            },
            960
        )
        .await
        .is_err());
    assert!(app
        .db_wrapper
        .orders()
        .get_all_orders()
        .await
        .unwrap()
        .is_empty());
    assert_no_further_gateway_messages(&server, 0).await;
    stop_app(app).await;
}

// --- Event subscriber -----------------------------------------------

const TIMESLOT: u64 = 1_767_225_600;

fn event(event_type: EwdsEventType, event_id: &str, data: Value) -> EwdsEventEnvelope<Value> {
    EwdsEventEnvelope {
        event_id: event_id.to_string(),
        event_type,
        occurred_at: epoch_to_rfc3339(TIMESLOT),
        data,
    }
}

fn facility(facility_id: &str, owner_id: &str) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: format!("Facility {}", facility_id),
        site_id: "site-1".to_string(),
        owner_id: owner_id.to_string(),
    }
}

fn measurement_data(facility_id: &str, time_slot: u64, energy_kwh: f64) -> Value {
    json!({
        "facilityId": facility_id,
        "communityUuid": "community-1",
        "timeSlot": epoch_to_rfc3339(time_slot),
        "creationTime": epoch_to_rfc3339(time_slot + 10),
        "energyKwh": energy_kwh,
    })
}

/// The stored timeseries values as (measurement point, timestamp, value), sorted.
async fn stored_values(app: &crate::helpers::TestApp) -> Vec<(String, String, f64)> {
    let mut values = app
        .db_wrapper
        .timeseries()
        .filter_values(None, None, None)
        .await
        .unwrap()
        .into_iter()
        .map(|value| (value.measurement_point, value.timestamp, value.value))
        .collect::<Vec<_>>();
    values.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    values
}

fn value(facility_id: &str, time_slot: u64, energy_kwh: f64) -> (String, String, f64) {
    (
        format!("measurement:community-1:{}", facility_id),
        timestamp_to_string_with_padding(time_slot),
        energy_kwh,
    )
}

#[tokio::test]
async fn handle_event_saves_facilities_sites_and_communities() {
    let app = init_app().await;
    let db = &app.db_wrapper;

    handle_event(
        db,
        event(
            EwdsEventType::Facility,
            "facility-event",
            json!([
                facility("facility-1", "owner-1"),
                facility("facility-2", "owner-2")
            ]),
        ),
    )
    .await
    .unwrap();
    handle_event(
        db,
        event(
            EwdsEventType::Site,
            "site-event",
            json!([{
                "site_name": "site-1",
                "site_description": "Main building",
                "facilities": ["facility-1", "facility-2"],
            }]),
        ),
    )
    .await
    .unwrap();
    handle_event(
        db,
        event(
            EwdsEventType::Community,
            "community-event",
            json!([{
                "communityId": "community-1",
                "communityName": "Community 1",
                "sites": ["site-1"],
            }]),
        ),
    )
    .await
    .unwrap();

    let mut facilities = db.facilities().get_all().await.unwrap();
    facilities.sort_by(|a, b| a.facility_id.cmp(&b.facility_id));
    assert_eq!(
        facilities,
        vec![
            facility("facility-1", "owner-1"),
            facility("facility-2", "owner-2")
        ]
    );
    assert_eq!(
        db.sites().get_all().await.unwrap(),
        vec![SiteSchema {
            site_name: "site-1".to_string(),
            site_description: "Main building".to_string(),
            facilities: vec!["facility-1".to_string(), "facility-2".to_string()],
        }]
    );
    assert_eq!(
        db.communities().get_all().await.unwrap(),
        vec![EnergyCommunitySchema {
            community_id: "community-1".to_string(),
            community_name: "Community 1".to_string(),
            sites: vec!["site-1".to_string()],
        }]
    );

    stop_app(app).await;
}

/// Sends batches of `(id, name)` records as `event_type` events and returns the `(id, name)`
/// pairs stored afterwards. The first batch is stored; every later one conflicts with it or
/// with itself, so it must be rejected without writing anything.
async fn assert_conflicting_batches_are_rejected<F, G>(
    event_type: EwdsEventType,
    record: F,
    stored: impl Fn(DatabaseWrapper) -> G,
) where
    F: Fn(&str, &str) -> Value,
    G: std::future::Future<Output = Vec<(String, String)>>,
{
    let app = init_app().await;
    let db = &app.db_wrapper;
    let batch = |records: &[(&str, &str)]| {
        Value::Array(records.iter().map(|(id, name)| record(id, name)).collect())
    };
    let initial = vec![("id-1".to_string(), "name-1".to_string())];

    handle_event(
        db,
        event(event_type, "initial", batch(&[("id-1", "name-1")])),
    )
    .await
    .unwrap();

    let conflicting: [&[(&str, &str)]; 4] = [
        // A stored id under another name.
        &[("id-1", "name-2")],
        // A stored name under another id, after a valid record that must not be written.
        &[("id-3", "name-3"), ("id-2", "name-1")],
        // One id with two names in the batch.
        &[("id-4", "name-4"), ("id-4", "name-5")],
        // One name with two ids in the batch.
        &[("id-5", "name-6"), ("id-6", "name-6")],
    ];
    for (index, records) in conflicting.into_iter().enumerate() {
        let error = handle_event(
            db,
            event(event_type, &format!("conflict-{index}"), batch(records)),
        )
        .await
        .expect_err("A conflicting batch must be rejected");
        assert!(is_invalid_event(&error), "{:#}", error);
        assert_eq!(stored(db.clone()).await, initial, "batch {:?}", records);
    }

    // Sending a stored record again is fine.
    handle_event(
        db,
        event(event_type, "repeat", batch(&[("id-1", "name-1")])),
    )
    .await
    .unwrap();
    assert_eq!(stored(db.clone()).await, initial);

    stop_app(app).await;
}

#[tokio::test]
async fn handle_event_rejects_facilities_that_conflict_with_stored_ids_or_names() {
    assert_conflicting_batches_are_rejected(
        EwdsEventType::Facility,
        |id, name| {
            json!(FacilitySchema {
                facility_name: name.to_string(),
                ..facility(id, "owner-1")
            })
        },
        |db| async move {
            let mut facilities = db.facilities().get_all().await.unwrap();
            facilities.sort_by(|a, b| a.facility_id.cmp(&b.facility_id));
            facilities
                .into_iter()
                .map(|facility| (facility.facility_id, facility.facility_name))
                .collect()
        },
    )
    .await;
}

#[tokio::test]
async fn handle_event_rejects_communities_that_conflict_with_stored_ids_or_names() {
    assert_conflicting_batches_are_rejected(
        EwdsEventType::Community,
        |id, name| json!({"communityId": id, "communityName": name, "sites": []}),
        |db| async move {
            let mut communities = db.communities().get_all().await.unwrap();
            communities.sort_by(|a, b| a.community_id.cmp(&b.community_id));
            communities
                .into_iter()
                .map(|community| (community.community_id, community.community_name))
                .collect()
        },
    )
    .await;
}

#[tokio::test]
async fn handle_event_saves_a_measurement_batch_across_facilities_and_slots() {
    let app = init_app().await;

    handle_event(
        &app.db_wrapper,
        event(
            EwdsEventType::Measurements,
            "measurements-event",
            json!([
                measurement_data("facility-1", TIMESLOT, 1.5),
                measurement_data("facility-1", TIMESLOT + 900, 2.0),
                measurement_data("facility-2", TIMESLOT, -0.5),
            ]),
        ),
    )
    .await
    .unwrap();

    let mut point_ids = app
        .db_wrapper
        .measurement_points()
        .filter_points(None, Some(MeasurementPointType::Measurement))
        .await
        .unwrap()
        .into_iter()
        .map(|point| point.measurement_id)
        .collect::<Vec<_>>();
    point_ids.sort();
    assert_eq!(
        point_ids,
        vec![
            "measurement:community-1:facility-1",
            "measurement:community-1:facility-2",
        ]
    );
    assert_eq!(
        stored_values(&app).await,
        vec![
            value("facility-1", TIMESLOT, 1.5),
            value("facility-1", TIMESLOT + 900, 2.0),
            value("facility-2", TIMESLOT, -0.5),
        ]
    );

    stop_app(app).await;
}

#[tokio::test]
async fn handle_event_stores_nothing_from_a_batch_with_an_invalid_measurement() {
    let app = init_app().await;
    let mut invalid = measurement_data("facility-1", TIMESLOT + 900, 2.0);
    invalid["timeSlot"] = json!(TIMESLOT + 900);

    let error = handle_event(
        &app.db_wrapper,
        event(
            EwdsEventType::Measurements,
            "measurements-event",
            json!([measurement_data("facility-1", TIMESLOT, 1.5), invalid]),
        ),
    )
    .await
    .unwrap_err();

    assert!(format!("{error:#}").contains("index 1"), "{error:#}");
    // Invalid data is not retried by the event worker.
    assert!(is_invalid_event(&error));
    assert!(stored_values(&app).await.is_empty());
    assert!(app
        .db_wrapper
        .measurement_points()
        .filter_points(None, None)
        .await
        .unwrap()
        .is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn handle_event_stores_nothing_from_a_batch_with_an_invalid_facility() {
    let app = init_app().await;

    let error = handle_event(
        &app.db_wrapper,
        event(
            EwdsEventType::Facility,
            "facility-event",
            json!([
                facility("facility-1", "owner-1"),
                {"facility_id": "facility-2"}
            ]),
        ),
    )
    .await
    .unwrap_err();

    assert!(format!("{error:#}").contains("index 1"), "{error:#}");
    assert!(is_invalid_event(&error));
    assert!(app
        .db_wrapper
        .facilities()
        .get_all()
        .await
        .unwrap()
        .is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn handle_event_updates_the_record_when_an_event_arrives_again() {
    let app = init_app().await;
    let db = &app.db_wrapper;

    for owner_id in ["owner-1", "owner-1", "owner-2"] {
        handle_event(
            db,
            event(
                EwdsEventType::Facility,
                "facility-event",
                json!([facility("facility-1", owner_id)]),
            ),
        )
        .await
        .unwrap();
    }
    for _ in 0..2 {
        handle_event(
            db,
            event(
                EwdsEventType::Measurements,
                "measurements-event",
                json!([measurement_data("facility-1", TIMESLOT, 1.5)]),
            ),
        )
        .await
        .unwrap();
    }

    assert_eq!(
        db.facilities().get_all().await.unwrap(),
        vec![facility("facility-1", "owner-2")]
    );
    assert_eq!(
        stored_values(&app).await,
        vec![value("facility-1", TIMESLOT, 1.5)]
    );

    stop_app(app).await;
}

#[tokio::test]
async fn handle_event_rejects_invalid_data_and_events_gsy_publishes() {
    let app = init_app().await;
    let db = &app.db_wrapper;

    assert!(handle_event(
        db,
        event(
            EwdsEventType::Facility,
            "facility-event",
            json!(facility("facility-1", "owner-1")),
        ),
    )
    .await
    .is_err());
    assert!(handle_event(
        db,
        event(EwdsEventType::Measurements, "measurements-event", json!([]),),
    )
    .await
    .is_err());
    assert!(
        handle_event(db, event(EwdsEventType::Trade, "trade-event", json!([])),)
            .await
            .is_err()
    );
    assert!(db.facilities().get_all().await.unwrap().is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn event_subscriber_skips_bad_messages_and_saves_the_next_one() {
    let app = init_app().await;
    let server = MockServer::start().await;
    // A site event on the facility topic must be skipped, not saved as a site.
    let wrong_topic_event = event(
        EwdsEventType::Site,
        "site-event",
        json!([{"site_name": "wrong-topic-site", "site_description": "", "facilities": []}]),
    );
    let facility_event = event(
        EwdsEventType::Facility,
        "facility-event",
        json!([facility("facility-1", "owner-1")]),
    );
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.events.sub"))
        .and(query_param("topicName", "facility"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"payload": "not an event"},
            {"payload": serde_json::to_string(&wrong_topic_event).unwrap()},
            {"payload": serde_json::to_string(&facility_event).unwrap()},
        ])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .with_priority(10)
        .mount(&server)
        .await;

    let subscriber = tokio::spawn(start_ewds_event_subscriber(
        app.db_wrapper.clone(),
        test_config(server.uri()),
    ));
    let mut facilities = Vec::new();
    for _ in 0..100 {
        facilities = app.db_wrapper.facilities().get_all().await.unwrap();
        if !facilities.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    subscriber.abort();

    assert_eq!(facilities, vec![facility("facility-1", "owner-1")]);
    assert!(app.db_wrapper.sites().get_all().await.unwrap().is_empty());

    stop_app(app).await;
}
