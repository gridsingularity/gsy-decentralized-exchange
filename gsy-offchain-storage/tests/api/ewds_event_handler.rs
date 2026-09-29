use crate::ewds_handler::{mock_gateway, test_config};
use crate::helpers::{init_app, stop_app};
use ethers::contract::LogMeta;
use ethers::types::{Address, H256, U256, U64};
use gsy_ethers_listener::{
    GsyEventHandler, MarketClearingFilter, MarketStatusUpdatedFilter, TradeSettledFilter,
};
use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
use gsy_offchain_storage::ewds_event_handler::{
    handle_event, start_ewds_event_subscriber, EwdsEventPublisher,
};
use gsy_offchain_storage::ewds_handler::EwdsHandlerConfig;
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema};
use primitives::db_api_schema::profiles::MeasurementPointType;
use primitives::db_api_schema::trades::{
    ClearingResultSchema, ClearingStatus, DbTradeSchema, TradeParameters, TradeStatus,
};
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsMarketStatusDto, EwdsSendMessageDto};
use primitives::ewds::{EwdsEventType, EwdsOperation};
use primitives::utils::{
    bytes16_to_hex, epoch_to_rfc3339, rfc3339_to_epoch, timestamp_to_string_with_padding,
};
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

fn trade(trade_uuid: &str, creation_time: u64) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: trade_uuid.to_string(),
        status: TradeStatus::Settled,
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
    }
}

#[tokio::test]
async fn publish_trades_created_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_trades_created(vec![trade("trade-1", 950)])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "tradeCreated");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::TradeCreated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(950)));
    assert_eq!(event["data"].as_array().unwrap().len(), 1);
    assert_eq!(event["data"][0]["tradeId"], json!("trade-1"));
    assert_eq!(event["data"][0]["marketId"], json!("market-1"));
}

#[tokio::test]
async fn publish_trades_created_sends_several_trades_in_one_event() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_trades_created(vec![trade("trade-1", 950), trade("trade-2", 960)])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    // occurredAt is the time of the newest trade.
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(960)));
    assert_eq!(event["data"][0]["tradeId"], json!("trade-1"));
    assert_eq!(event["data"][1]["tradeId"], json!("trade-2"));
}

#[tokio::test]
async fn every_event_gets_its_own_id() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    for _ in 0..2 {
        publisher
            .publish_trades_created(vec![trade("trade-1", 950)])
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
async fn publish_clearing_results_created_sends_event_on_events_channel() {
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
        .publish_clearing_results_created(vec![clearing_result])
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "clearingResultCreated");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::ClearingResultCreated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_100)));
    assert_eq!(event["data"][0]["marketId"], json!("market-1"));
    assert_eq!(event["data"][0]["numTrades"], json!(2));
    assert_eq!(event["data"][0]["txHash"], json!("0xabc"));
}

#[tokio::test]
async fn publish_market_statuses_updated_sends_event_on_events_channel() {
    let server = mock_gateway().await;
    let publisher = EwdsEventPublisher::new(test_config(server.uri()));

    publisher
        .publish_market_statuses_updated(
            vec![EwdsMarketStatusDto {
                market_id: "market-1".to_string(),
                is_open: false,
            }],
            1_200,
        )
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(send_dto.fqcn, "gsy.events.pub");
    assert_eq!(send_dto.topic_name, "marketStatusUpdated");

    let event: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_event_id(&send_dto, &event);
    assert_eq!(
        event["eventType"],
        json!(EwdsEventType::MarketStatusUpdated.as_str())
    );
    assert_eq!(event["occurredAt"], json!(epoch_to_rfc3339(1_200)));
    assert_eq!(
        event["data"],
        json!([{"marketId": "market-1", "isOpen": false}])
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
        .publish_market_statuses_updated(
            vec![EwdsMarketStatusDto {
                market_id: "market-1".to_string(),
                is_open: true,
            }],
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
            config.event_topic(EwdsEventType::TradeCreated),
            "tradeCreated",
        ),
        (
            "EWDS_CLEARING_RESULT_CREATED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::ClearingResultCreated),
            "clearingResultCreated",
        ),
        (
            "EWDS_MARKET_STATUS_UPDATED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::MarketStatusUpdated),
            "marketStatusUpdated",
        ),
        (
            "EWDS_MEASUREMENTS_SUBMITTED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::MeasurementsSubmitted),
            "measurementsSubmitted",
        ),
        (
            "EWDS_FACILITY_SUBMITTED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::FacilitySubmitted),
            "facilitySubmitted",
        ),
        (
            "EWDS_SITE_SUBMITTED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::SiteSubmitted),
            "siteSubmitted",
        ),
        (
            "EWDS_COMMUNITY_SUBMITTED_EVENT_TOPIC",
            config.event_topic(EwdsEventType::CommunitySubmitted),
            "communitySubmitted",
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
    assert_event_id(&messages[0], &event);
    assert_eq!(event["data"].as_array().unwrap().len(), 1);
    assert_eq!(event["data"][0]["tradeId"], json!(trade_id));
    assert_eq!(event["data"][0]["bidId"], json!(bytes16_to_hex([0x02; 16])));
    assert_eq!(
        event["data"][0]["offerId"],
        json!(bytes16_to_hex([0x03; 16]))
    );
    assert_eq!(event["data"][0]["tradeStatus"], json!("settled"));
    assert_eq!(event["data"][0]["residualBidId"], json!(null));

    let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(
        event["occurredAt"],
        json!(epoch_to_rfc3339(stored[0].creation_time))
    );

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
    assert_event_id(&messages[0], &event);
    assert_eq!(
        event["data"],
        json!([{"marketId": market_id, "isOpen": true}])
    );
    // Market status is not persisted, so occurredAt is the time the event was received.
    let occurred_at = rfc3339_to_epoch(event["occurredAt"].as_str().unwrap()).unwrap();
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
        (
            EwdsEventType::MeasurementsSubmitted,
            "measurementsSubmitted",
        ),
        (EwdsEventType::FacilitySubmitted, "facilitySubmitted"),
        (EwdsEventType::SiteSubmitted, "siteSubmitted"),
        (EwdsEventType::CommunitySubmitted, "communitySubmitted"),
    ] {
        assert_eq!(config.event_topic(event_type), topic);
    }
}

#[test]
fn handler_config_maps_topics_to_their_subscribe_channels() {
    let config = test_config("http://gateway".to_string());
    for operation in EwdsOperation::ALL {
        let topic = config.topics.for_operation(operation).request.as_str();
        assert_eq!(
            config.subscribe_fqcn(topic),
            "gsy.requests.sub",
            "{}",
            topic
        );
    }
    for event_type in EwdsEventType::ALL {
        let topic = config.event_topic(event_type);
        assert_eq!(config.subscribe_fqcn(topic), "gsy.events.sub", "{}", topic);
    }
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
            EwdsEventType::FacilitySubmitted,
            "facility-event",
            json!(facility("facility-1", "owner-1")),
        ),
    )
    .await
    .unwrap();
    handle_event(
        db,
        event(
            EwdsEventType::SiteSubmitted,
            "site-event",
            json!({
                "site_name": "site-1",
                "site_description": "Main building",
                "facilities": ["facility-1"],
            }),
        ),
    )
    .await
    .unwrap();
    handle_event(
        db,
        event(
            EwdsEventType::CommunitySubmitted,
            "community-event",
            json!({
                "communityId": "community-1",
                "communityName": "Community 1",
                "sites": ["site-1"],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        db.facilities().get_all().await.unwrap(),
        vec![facility("facility-1", "owner-1")]
    );
    assert_eq!(
        db.sites().get_all().await.unwrap(),
        vec![SiteSchema {
            site_name: "site-1".to_string(),
            site_description: "Main building".to_string(),
            facilities: vec!["facility-1".to_string()],
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

#[tokio::test]
async fn handle_event_saves_a_measurement_batch_across_facilities_and_slots() {
    let app = init_app().await;

    handle_event(
        &app.db_wrapper,
        event(
            EwdsEventType::MeasurementsSubmitted,
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
            EwdsEventType::MeasurementsSubmitted,
            "measurements-event",
            json!([measurement_data("facility-1", TIMESLOT, 1.5), invalid]),
        ),
    )
    .await
    .unwrap_err();

    assert!(format!("{error:#}").contains("index 1"), "{error:#}");
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
async fn handle_event_updates_the_record_when_an_event_arrives_again() {
    let app = init_app().await;
    let db = &app.db_wrapper;

    for owner_id in ["owner-1", "owner-1", "owner-2"] {
        handle_event(
            db,
            event(
                EwdsEventType::FacilitySubmitted,
                "facility-event",
                json!(facility("facility-1", owner_id)),
            ),
        )
        .await
        .unwrap();
    }
    for _ in 0..2 {
        handle_event(
            db,
            event(
                EwdsEventType::MeasurementsSubmitted,
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
            EwdsEventType::FacilitySubmitted,
            "facility-event",
            json!({"facility_id": "facility-1"}),
        ),
    )
    .await
    .is_err());
    assert!(handle_event(
        db,
        event(
            EwdsEventType::MeasurementsSubmitted,
            "measurements-event",
            json!([]),
        ),
    )
    .await
    .is_err());
    assert!(handle_event(
        db,
        event(EwdsEventType::TradeCreated, "trade-event", json!([])),
    )
    .await
    .is_err());
    assert!(db.facilities().get_all().await.unwrap().is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn event_subscriber_skips_bad_messages_and_saves_the_next_one() {
    let app = init_app().await;
    let server = MockServer::start().await;
    // A site event on the facility topic must be skipped, not saved as a site.
    let wrong_topic_event = event(
        EwdsEventType::SiteSubmitted,
        "site-event",
        json!({"site_name": "wrong-topic-site", "site_description": "", "facilities": []}),
    );
    let facility_event = event(
        EwdsEventType::FacilitySubmitted,
        "facility-event",
        json!(facility("facility-1", "owner-1")),
    );
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .and(query_param("fqcn", "gsy.events.sub"))
        .and(query_param("topicName", "facilitySubmitted"))
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
