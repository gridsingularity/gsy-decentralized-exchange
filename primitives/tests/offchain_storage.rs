use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, MeasurementSchema,
    TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus};
use primitives::ewds::dto::{
    EwdsCommunityDto, EwdsMarketDto, EwdsMeasurementDto, EwdsOrderDto, EwdsTradeDto,
};
use primitives::offchain_storage::{
    CommunityProvider, OffchainStorageClient, OffchainStorageTransport,
};
use primitives::utils::{epoch_to_rfc3339, timestamp_to_string_with_padding};
use serde_json::{json, Value};
use std::env;
use std::sync::{Arc, Mutex};
use wiremock::http::Method;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

// Env vars are process-global; serialize tests that mutate them.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn base_facility(facility_id: &str, owner_id: &str) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: facility_id.to_string(),
        site_id: "site 1".to_string(),
        owner_id: owner_id.to_string(),
    }
}

/// Mounts the EWDS gateway's send/poll endpoints on `server`, answering every
/// `operation` request with `response_data`.
async fn mount_ewds_query(server: &MockServer, operation: &'static str, response_data: Value) {
    let pending = Arc::new(Mutex::new(Value::Null));
    let sent = pending.clone();
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let envelope: Value = serde_json::from_str(body["payload"].as_str().unwrap()).unwrap();
            assert_eq!(envelope["operation"], operation);
            *sent.lock().unwrap() = json!({
                "requestId": envelope["requestId"],
                "success": true,
                "data": response_data,
            });
            ResponseTemplate::new(200).set_body_json(json!({
                "recipients": {"sent": 1, "failed": 0, "total": 1}
            }))
        })
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!([
                {"payload": pending.lock().unwrap().to_string()}
            ]))
        })
        .mount(server)
        .await;
}

fn set_ewds_env(server: &MockServer) {
    env::set_var("OFFCHAIN_STORAGE_TRANSPORT", "ewds");
    env::set_var("EWDS_GATEWAY_URL", server.uri());
    env::set_var("EWDS_RESPONSE_TIMEOUT_MS", "1000");
}

fn clear_ewds_env() {
    env::remove_var("OFFCHAIN_STORAGE_TRANSPORT");
    env::remove_var("EWDS_GATEWAY_URL");
    env::remove_var("EWDS_RESPONSE_TIMEOUT_MS");
}

// -- FacilityOwnerProvider -------------------------------------------------

#[tokio::test]
async fn fetches_facility_owner_mapping_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let facilities = vec![
        base_facility("AIS1-House-1", "owner 1"),
        base_facility("AIS1-House-2", "owner 2"),
    ];
    Mock::given(method("GET"))
        .and(path("/facilities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&facilities))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let mapping = client.fetch_facility_owner_mapping().await.unwrap();

    assert_eq!(mapping.len(), 2);
    assert_eq!(
        mapping.get("AIS1-House-1").map(String::as_str),
        Some("owner 1")
    );
    assert_eq!(
        mapping.get("AIS1-House-2").map(String::as_str),
        Some("owner 2")
    );
}

#[tokio::test]
async fn empty_facilities_yields_empty_mapping() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let empty: Vec<FacilitySchema> = vec![];
    Mock::given(method("GET"))
        .and(path("/facilities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&empty))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let mapping = client.fetch_facility_owner_mapping().await.unwrap();

    assert!(mapping.is_empty());
}

#[tokio::test]
async fn errors_on_non_success_facilities_status() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/facilities"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let err = client.fetch_facility_owner_mapping().await.unwrap_err();

    assert!(err.to_string().contains("Failed to fetch facilities"));
}

#[tokio::test]
async fn fetches_facility_owner_mapping_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let facilities = vec![
        base_facility("AIS1-House-1", "owner 1"),
        base_facility("AIS1-House-2", "owner 2"),
    ];
    mount_ewds_query(&server, "facilities.query", json!(facilities)).await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testfacilities",
    );
    let mapping = client.fetch_facility_owner_mapping().await.unwrap();

    assert_eq!(mapping.len(), 2);
    assert_eq!(
        mapping.get("AIS1-House-1").map(String::as_str),
        Some("owner 1")
    );

    clear_ewds_env();
}

// -- IdMappingProvider -------------------------------------------------------

fn id_mapping(offchain_id: &str, onchain_id: &str) -> IdMappingSchema {
    IdMappingSchema {
        offchain_id: offchain_id.to_string(),
        onchain_id: onchain_id.to_string(),
        creation_time: 1,
    }
}

#[tokio::test]
async fn fetches_onchain_id_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let mapping = id_mapping(
        "00112233-4455-6677-8899-aabbccddeeff",
        "0x11111111111111111111111111111111",
    );
    Mock::given(method("POST"))
        .and(path("/ids"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&mapping))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let onchain_id = client
        .fetch_onchain_id("00112233-4455-6677-8899-aabbccddeeff")
        .await
        .unwrap();

    assert_eq!(onchain_id, "0x11111111111111111111111111111111");
}

#[tokio::test]
async fn errors_when_id_mapping_is_for_a_different_facility() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let mapping = id_mapping("someone-else", "0x11111111111111111111111111111111");
    Mock::given(method("POST"))
        .and(path("/ids"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&mapping))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let err = client
        .fetch_onchain_id("00112233-4455-6677-8899-aabbccddeeff")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("mapping for a different facility"));
}

#[tokio::test]
async fn fetches_onchain_id_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let mapping = id_mapping(
        "00112233-4455-6677-8899-aabbccddeeff",
        "0x22222222222222222222222222222222",
    );
    mount_ewds_query(&server, "ids.query", json!([mapping])).await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testids",
    );
    let onchain_id = client
        .fetch_onchain_id("00112233-4455-6677-8899-aabbccddeeff")
        .await
        .unwrap();

    assert_eq!(onchain_id, "0x22222222222222222222222222222222");

    clear_ewds_env();
}

// -- CommunityProvider (moved from gsy-market-orchestrator) -----------------

#[tokio::test]
async fn fetches_communities_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let communities = vec![
        EnergyCommunitySchema {
            community_id: "11111111-1111-4111-8111-111111111111".to_string(),
            community_name: "Community One".to_string(),
            sites: vec!["site-one".to_string()],
        },
        EnergyCommunitySchema {
            community_id: "22222222-2222-4222-8222-222222222222".to_string(),
            community_name: "Community Two".to_string(),
            sites: vec!["site-two".to_string()],
        },
    ];
    Mock::given(method("GET"))
        .and(path("/communities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&communities))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let fetched = client.fetch_communities().await.unwrap();

    assert_eq!(fetched.len(), 2);
    assert_eq!(
        fetched[0].community_id,
        "11111111-1111-4111-8111-111111111111"
    );
    assert_eq!(
        fetched[1].community_id,
        "22222222-2222-4222-8222-222222222222"
    );
}

#[tokio::test]
async fn propagates_community_http_failures() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/communities"))
        .respond_with(ResponseTemplate::new(503).set_body_string("temporarily unavailable"))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let error = client.fetch_communities().await.unwrap_err().to_string();

    assert!(error.contains("HTTP 503 Service Unavailable"));
    assert!(error.contains("temporarily unavailable"));
}

#[tokio::test]
async fn propagates_community_deserialization_failures() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/communities"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"community_id":"invalid"}"#))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let error = client.fetch_communities().await.unwrap_err().to_string();

    assert!(error.contains("Failed to deserialize communities"));
}

#[tokio::test]
async fn fetches_communities_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    // EwdsCommunityDto is #[serde(rename_all = "camelCase")], unlike
    // EnergyCommunitySchema, so the EWDS response fixture must use the DTO.
    let communities = vec![EwdsCommunityDto {
        community_id: "11111111-1111-4111-8111-111111111111".to_string(),
        community_name: "Community One".to_string(),
        sites: vec!["site-one".to_string()],
    }];
    mount_ewds_query(&server, "communities.query", json!(communities)).await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testcommunities",
    );
    let fetched = client.fetch_communities().await.unwrap();

    assert_eq!(fetched.len(), 1);
    assert_eq!(
        fetched[0].community_id,
        "11111111-1111-4111-8111-111111111111"
    );

    clear_ewds_env();
}

// -- Trades and measurements (moved from gsy-execution-engine) --------------

const TIMESLOT: u64 = 1_767_225_600;
const TIMESLOT_END: u64 = TIMESLOT + 899;

fn trade() -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: "trade-1".to_string(),
        status: TradeStatus::Settled,
        seller: "seller-1".to_string(),
        buyer: "buyer-1".to_string(),
        market_id: "market-1".to_string(),
        creation_time: TIMESLOT + 60,
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

fn measurement() -> MeasurementSchema {
    MeasurementSchema {
        facility_id: "facility-1".to_string(),
        community_uuid: "community-1".to_string(),
        time_slot: TIMESLOT,
        creation_time: TIMESLOT,
        energy_kwh: 1.5,
    }
}

/// The query payload of the EWDS request the client sent to `server`.
async fn sent_ewds_query(server: &MockServer) -> Value {
    let requests = server.received_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|request| request.method == Method::POST)
        .expect("no EWDS request was sent");
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    let envelope: Value = serde_json::from_str(body["payload"].as_str().unwrap()).unwrap();
    envelope["payload"].clone()
}

#[tokio::test]
async fn fetches_trades_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/trades"))
        .and(query_param("start_time", epoch_to_rfc3339(TIMESLOT)))
        .and(query_param("end_time", epoch_to_rfc3339(TIMESLOT_END)))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![EwdsTradeDto::from(trade())]))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let trades = client
        .fetch_trades(None, Some(TIMESLOT), Some(TIMESLOT_END))
        .await
        .unwrap();

    assert_eq!(trades, vec![trade()]);
}

#[tokio::test]
async fn fetches_trades_over_ewds_with_rfc3339_range() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    mount_ewds_query(
        &server,
        "trades.query",
        json!([EwdsTradeDto::from(trade())]),
    )
    .await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testtrades",
    );
    let trades = client
        .fetch_trades(None, Some(TIMESLOT), Some(TIMESLOT_END))
        .await
        .unwrap();

    assert_eq!(trades, vec![trade()]);
    assert_eq!(
        sent_ewds_query(&server).await,
        json!({
            "startTime": epoch_to_rfc3339(TIMESLOT),
            "endTime": epoch_to_rfc3339(TIMESLOT_END),
        })
    );

    clear_ewds_env();
}

#[tokio::test]
async fn fetches_trades_by_market_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/trades"))
        .and(query_param("market_id", "market-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![EwdsTradeDto::from(trade())]))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let trades = client
        .fetch_trades(Some("market-1"), None, None)
        .await
        .unwrap();

    assert_eq!(trades, vec![trade()]);
}

#[tokio::test]
async fn fetches_trades_by_market_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    mount_ewds_query(
        &server,
        "trades.query",
        json!([EwdsTradeDto::from(trade())]),
    )
    .await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testmarkettrades",
    );
    let trades = client
        .fetch_trades(Some("market-1"), None, None)
        .await
        .unwrap();

    assert_eq!(trades, vec![trade()]);
    assert_eq!(
        sent_ewds_query(&server).await,
        json!({ "marketId": "market-1" })
    );

    clear_ewds_env();
}

fn order_dto() -> EwdsOrderDto {
    EwdsOrderDto::from(DbOrderSchema {
        order_id: "order-1".to_string(),
        status: OrderStatus::Submitted,
        order_type: OrderEnum::Bid,
        area_uuid: "buyer-1".to_string(),
        market_id: "market-1".to_string(),
        time_slot: TIMESLOT,
        creation_time: TIMESLOT - 60,
        energy_kWh: 2.0,
        energy_rate: 0.3,
        created_by: "buyer-1".to_string(),
        requirements: None,
        attributes: None,
    })
}

#[tokio::test]
async fn fetches_orders_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/orders"))
        .and(query_param("market_id", "market-1"))
        .and(query_param("start_time", epoch_to_rfc3339(TIMESLOT)))
        .and(query_param("end_time", epoch_to_rfc3339(TIMESLOT_END)))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![order_dto()]))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let orders = client
        .fetch_orders("market-1", TIMESLOT, TIMESLOT_END)
        .await
        .unwrap();

    assert_eq!(orders, vec![DbOrderSchema::try_from(order_dto()).unwrap()]);
}

#[tokio::test]
async fn fetches_orders_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    mount_ewds_query(&server, "orders.query", json!([order_dto()])).await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testorders",
    );
    let orders = client
        .fetch_orders("market-1", TIMESLOT, TIMESLOT_END)
        .await
        .unwrap();

    assert_eq!(orders, vec![DbOrderSchema::try_from(order_dto()).unwrap()]);
    assert_eq!(
        sent_ewds_query(&server).await,
        json!({
            "marketId": "market-1",
            "startTime": epoch_to_rfc3339(TIMESLOT),
            "endTime": epoch_to_rfc3339(TIMESLOT_END),
        })
    );

    clear_ewds_env();
}

#[tokio::test]
async fn fetches_measurements_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    let measurement_id = "measurement:community-1:facility-1".to_string();
    let point = MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: measurement_id.clone(),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: FlowDirection::Import,
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: "facility-1".to_string(),
        datasource_name: Some("community-1".to_string()),
    };
    let value = TimeseriesSchema {
        measurement_point: measurement_id,
        timestamp: timestamp_to_string_with_padding(TIMESLOT),
        value: 1.5,
    };
    Mock::given(method("GET"))
        .and(path("/measurement-points"))
        .and(query_param("type", "Measurement"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![point]))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/timeseries"))
        .and(query_param(
            "start_time",
            timestamp_to_string_with_padding(TIMESLOT),
        ))
        .and(query_param(
            "end_time",
            timestamp_to_string_with_padding(TIMESLOT_END),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![value]))
        .mount(&server)
        .await;

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    );
    let measurements = client
        .fetch_measurements(TIMESLOT, TIMESLOT_END)
        .await
        .unwrap();

    assert_eq!(measurements, vec![measurement()]);
}

#[tokio::test]
async fn fetches_measurements_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    // measurements.query answers with EwdsMeasurementDto, whose times are RFC 3339 strings.
    mount_ewds_query(
        &server,
        "measurements.query",
        json!([EwdsMeasurementDto::from(measurement())]),
    )
    .await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testmeasurements",
    );
    let measurements = client
        .fetch_measurements(TIMESLOT, TIMESLOT_END)
        .await
        .unwrap();

    assert_eq!(measurements, vec![measurement()]);
    assert_eq!(
        sent_ewds_query(&server).await,
        json!({
            "startTime": epoch_to_rfc3339(TIMESLOT),
            "endTime": epoch_to_rfc3339(TIMESLOT_END),
        })
    );

    clear_ewds_env();
}

// -- Markets ---------------------------------------------------------------

const MARKET_ID: &str = "0xcccccccccccccccccccccccccccccccc";

fn market() -> MarketSchema {
    MarketSchema {
        market_id: MARKET_ID.to_string(),
        community_id: "11111111-1111-4111-8111-111111111111".to_string(),
        opening_time: "00000000001699998300".to_string(),
        closing_time: "00000000001700000100".to_string(),
        delivery_start_time: "00000000001700000100".to_string(),
        delivery_end_time: "00000000001700001000".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: "00000000001699998301".to_string(),
    }
}

fn http_client(server: &MockServer) -> OffchainStorageClient {
    OffchainStorageClient::new(
        OffchainStorageTransport::Http,
        server.uri(),
        "UNUSED_ENV",
        "unused-default",
    )
}

#[tokio::test]
async fn fetches_market_over_http() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/market"))
        .and(query_param("market_id", MARKET_ID))
        .respond_with(ResponseTemplate::new(200).set_body_json(market()))
        .mount(&server)
        .await;

    let fetched = http_client(&server).fetch_market(MARKET_ID).await.unwrap();

    assert_eq!(fetched, Some(market()));
}

#[tokio::test]
async fn missing_market_over_http_is_none() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/market"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let fetched = http_client(&server).fetch_market(MARKET_ID).await.unwrap();

    assert_eq!(fetched, None);
}

#[tokio::test]
async fn propagates_market_http_failures() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/market"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let error = http_client(&server)
        .fetch_market(MARKET_ID)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("HTTP 500"));
}

#[tokio::test]
async fn fetches_market_over_ewds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    mount_ewds_query(
        &server,
        "markets.query",
        json!([EwdsMarketDto::from(market())]),
    )
    .await;
    set_ewds_env(&server);

    let client = OffchainStorageClient::new(
        OffchainStorageTransport::Ewds,
        server.uri(),
        "EWDS_TEST_CLIENT_ID",
        "testmarkets",
    );
    let fetched = client.fetch_market(MARKET_ID).await.unwrap();

    assert_eq!(fetched, Some(market()));

    clear_ewds_env();
}
