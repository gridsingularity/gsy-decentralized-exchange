use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::ewds_handler::{handle_request, EwdsHandlerConfig};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::grid_topology::FacilitySchema;
use primitives::ewds::dto::{EwdsRequestEnvelope, EwdsSendMessageDto};
use primitives::ewds::{EwdsOperation, EwdsTopicConfig};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
// --- Test helpers ---------------------------------------------------

fn test_config(gateway_url: String) -> EwdsHandlerConfig {
    EwdsHandlerConfig {
        enabled: true,
        gateway_url,
        request_fqcn: "gsy.requests.sub".to_string(),
        response_fqcn: "gsy.responses.pub".to_string(),
        topic_owner: "test.owner".to_string(),
        topic_version: "1.0.0".to_string(),
        request_client_id: "gsyoffchainstorage".to_string(),
        topics: EwdsTopicConfig::from_env(),
        poll_interval_ms: 500,
        request_batch_size: 100,
    }
}

fn envelope(
    operation: EwdsOperation,
    request_id: &str,
    payload: serde_json::Value,
) -> EwdsRequestEnvelope {
    EwdsRequestEnvelope {
        request_id: request_id.to_string(),
        operation,
        payload,
    }
}

async fn mock_gateway() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    server
}

/// Parse the single captured POST body into the response envelope's data array.
async fn captured_data(server: &MockServer) -> Vec<serde_json::Value> {
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&send_dto.payload).unwrap();
    assert_eq!(envelope["success"], json!(true));
    envelope["data"].as_array().unwrap().clone()
}

// --- OrdersQuery ----------------------------------------------------

#[tokio::test]
async fn orders_query_success() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // seed one order for market "m1" if desired:
    // app.db_wrapper.orders().insert_orders(vec![sample_order("m1")]).await.unwrap();

    let env = envelope(
        EwdsOperation::OrdersQuery,
        "req-orders-1",
        json!({ "marketId": "m1" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let _data = captured_data(&server).await;

    stop_app(app).await;
}

#[tokio::test]
async fn orders_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::OrdersQuery,
        "req-orders-bad",
        json!({ "startTime": "not-a-number" }),
    );

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("orders.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}

// --- TradesQuery ----------------------------------------------------

#[tokio::test]
async fn trades_query_success() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::TradesQuery,
        "req-trades-1",
        json!({ "startTime": 0, "endTime": 9_999_999_999u64 }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();
    let _data = captured_data(&server).await;

    stop_app(app).await;
}

#[tokio::test]
async fn trades_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::TradesQuery,
        "req-trades-bad",
        json!({ "endTime": "nope" }),
    );

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("trades.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}

// --- MeasurementsQuery ----------------------------------------------

#[tokio::test]
async fn measurements_query_success() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::MeasurementsQuery,
        "req-meas-1",
        json!({ "startTime": 0, "endTime": 9_999_999_999u64, "areaUuid": "facility-1" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();
    let _data = captured_data(&server).await;

    stop_app(app).await;
}

#[tokio::test]
async fn measurements_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::MeasurementsQuery,
        "req-meas-bad",
        json!({ "startTime": "x" }),
    );

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("measurements.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}

// --- ClearingResultsQuery -------------------------------------------

#[tokio::test]
async fn clearing_results_query_success() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // seed a clearing result for "m1" if desired:
    // app.db_wrapper.clearing_results().insert(sample_clearing_result("m1")).await.unwrap();

    let env = envelope(
        EwdsOperation::ClearingResultsQuery,
        "req-clearing-1",
        json!({ "marketId": "m1" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();
    let _data = captured_data(&server).await;

    stop_app(app).await;
}

#[tokio::test]
async fn clearing_results_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // market_id is required -> missing field errors
    let env = envelope(
        EwdsOperation::ClearingResultsQuery,
        "req-clearing-bad",
        json!({}),
    );

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("clearing_results.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}


fn make_market(market_id: &str, community_id: &str, opening_time: &str) -> MarketSchema {
    MarketSchema {
        market_id: market_id.to_string(),
        community_id: community_id.to_string(),
        opening_time: opening_time.to_string(),
        closing_time: "2026-03-28T09:45:00Z".to_string(),
        delivery_start_time: "2026-03-28T10:00:00Z".to_string(),
        delivery_end_time: "2026-03-28T10:15:00Z".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: "2026-03-28T09:45:00Z".to_string(),
    }
}

#[tokio::test]
async fn markets_query_filters_by_community_and_serialises_camel_case() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let markets = app.db_wrapper.markets();
    markets
        .upsert(make_market("m1", "community1", "2026-03-27T18:00:00Z"))
        .await
        .unwrap();
    markets
        .upsert(make_market("m2", "community2", "2026-03-27T19:00:00Z"))
        .await
        .unwrap();

    let env = envelope(
        EwdsOperation::MarketsQuery,
        "req-markets-1",
        json!({ "communityId": "community1" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["marketId"], json!("m1"));
    assert_eq!(data[0]["communityId"], json!("community1"));
    assert_eq!(data[0]["deliveryStartTime"], json!("2026-03-28T10:00:00Z"));
    assert_eq!(data[0]["marketType"], json!("spot"));
    assert_eq!(data[0]["matchingAlgorithm"], json!("pay_as_bid"));

    stop_app(app).await;
}

#[tokio::test]
async fn markets_query_filters_by_opening_time_window() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let markets = app.db_wrapper.markets();
    markets
        .upsert(make_market("early", "community1", "2026-03-27T18:00:00Z"))
        .await
        .unwrap();
    markets
        .upsert(make_market("late", "community1", "2026-03-27T22:00:00Z"))
        .await
        .unwrap();

    let env = envelope(
        EwdsOperation::MarketsQuery,
        "req-markets-window",
        json!({ "startTime": "2026-03-27T20:00:00Z", "endTime": "2026-03-27T23:00:00Z" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["marketId"], json!("late"));

    stop_app(app).await;
}

#[tokio::test]
async fn markets_query_without_filters_returns_all() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let markets = app.db_wrapper.markets();
    markets
        .upsert(make_market("m1", "community1", "2026-03-27T18:00:00Z"))
        .await
        .unwrap();
    markets
        .upsert(make_market("m2", "community2", "2026-03-27T19:00:00Z"))
        .await
        .unwrap();

    let env = envelope(EwdsOperation::MarketsQuery, "req-markets-all", json!({}));

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    assert_eq!(data.len(), 2);

    stop_app(app).await;
}

#[tokio::test]
async fn markets_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // market_id is a string field -> a number errors
    let env = envelope(
        EwdsOperation::MarketsQuery,
        "req-markets-bad",
        json!({ "marketId": 5 }),
    );

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("markets.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}

// --- FacilitiesQuery ------------------------------------------------

fn make_facility(facility_id: &str, facility_name: &str) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: facility_name.to_string(),
        site_id: "site-1".to_string(),
        owner_id: "owner-1".to_string(),
    }
}

#[tokio::test]
async fn facilities_query_returns_all() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let facilities = app.db_wrapper.facilities();
    facilities
        .insert(make_facility("f1", "facility-1"))
        .await
        .unwrap();
    facilities
        .insert(make_facility("f2", "facility-2"))
        .await
        .unwrap();

    let env = envelope(EwdsOperation::FacilitiesQuery, "req-facilities-1", json!({}));

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    assert_eq!(data.len(), 2);

    let ids: Vec<&str> = data
        .iter()
        .map(|f| f["facility_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"f1"));
    assert!(ids.contains(&"f2"));

    stop_app(app).await;
}

#[tokio::test]
async fn facilities_query_empty_returns_empty() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::FacilitiesQuery,
        "req-facilities-empty",
        json!({}),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    assert!(data.is_empty());

    stop_app(app).await;
}