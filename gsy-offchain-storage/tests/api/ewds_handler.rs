use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::ewds_handler::{
    handle_request, validate_trades_query_range, EwdsHandlerConfig, INVALID_TIME_RANGE,
    MAX_TRADES_QUERY_RANGE_SECS, TIME_RANGE_TOO_LARGE,
};
use primitives::db_api_schema::grid_topology::FacilitySchema;
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::ewds::dto::{EwdsRequestEnvelope, EwdsSendMessageDto, EwdsTradeDto};
use primitives::ewds::{EwdsOperation, EwdsTopicConfig};
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};
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
        response_send_timeout_ms: 1_000,
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
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "recipients": {"failed": 0, "sent": 1, "total": 1}
        })))
        .mount(&server)
        .await;
    server
}

/// Parse the single captured POST body into the response envelope.
async fn captured_envelope(server: &MockServer) -> serde_json::Value {
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected exactly one gateway POST");
    let send_dto: EwdsSendMessageDto = serde_json::from_slice(&requests[0].body).unwrap();
    serde_json::from_str(&send_dto.payload).unwrap()
}

/// Parse the single captured POST body into the response envelope's data array.
async fn captured_data(server: &MockServer) -> Vec<serde_json::Value> {
    let envelope = captured_envelope(server).await;
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

fn make_trade(trade_id: &str, timestamp: u64) -> DbTradeSchema {
    DbTradeSchema::try_from(EwdsTradeDto {
        trade_id: trade_id.to_string(),
        market_id: "m1".to_string(),
        bid_id: format!("{}-bid", trade_id),
        buyer_id: "Load1".to_string(),
        residual_bid_id: None,
        offer_id: format!("{}-offer", trade_id),
        seller_id: "PV1".to_string(),
        residual_offer_id: None,
        trade_status: "settled".to_string(),
        trade_quantity: 1.5,
        trade_price: 0.12,
        timestamp,
    })
    .unwrap()
}

/// Run a trades.query with the given payload against a DB holding one trade
/// at the start of the day and one at the next day, returning the response.
async fn run_trades_query(request_id: &str, payload: serde_json::Value) -> serde_json::Value {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    app.db_wrapper
        .trades()
        .insert_trades(vec![
            make_trade("trade-day-start", 0),
            make_trade("trade-next-day", MAX_TRADES_QUERY_RANGE_SECS),
        ])
        .await
        .unwrap();

    let env = envelope(EwdsOperation::TradesQuery, request_id, payload);
    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();
    let response = captured_envelope(&server).await;

    stop_app(app).await;
    response
}

fn assert_trades_query_rejected(response: &serde_json::Value, request_id: &str, code: &str) {
    assert_eq!(response["requestId"], json!(request_id));
    assert_eq!(response["success"], json!(false));
    assert_eq!(response["data"], json!([]));
    assert_eq!(response["error"]["code"], json!(code));
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("trades.query"));
}

#[tokio::test]
async fn trades_query_success() {
    // Exactly one day is the largest allowed range; endTime is exclusive.
    let response = run_trades_query(
        "req-trades-1",
        json!({ "startTime": 0, "endTime": MAX_TRADES_QUERY_RANGE_SECS }),
    )
    .await;

    assert_eq!(response["success"], json!(true));
    let ids: Vec<&str> = response["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|trade| trade["tradeId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["trade-day-start"]);
}

#[tokio::test]
async fn trades_query_range_over_one_day_publishes_error() {
    let response = run_trades_query(
        "req-trades-too-large",
        json!({ "startTime": 0, "endTime": MAX_TRADES_QUERY_RANGE_SECS + 1 }),
    )
    .await;

    assert_trades_query_rejected(&response, "req-trades-too-large", TIME_RANGE_TOO_LARGE);
}

#[tokio::test]
async fn trades_query_missing_start_time_publishes_error() {
    let response = run_trades_query("req-trades-no-start", json!({ "endTime": 3_600 })).await;

    assert_trades_query_rejected(&response, "req-trades-no-start", INVALID_TIME_RANGE);
}

#[tokio::test]
async fn trades_query_inverted_range_publishes_error() {
    let response = run_trades_query(
        "req-trades-inverted",
        json!({ "startTime": 3_600, "endTime": 0 }),
    )
    .await;

    assert_trades_query_rejected(&response, "req-trades-inverted", INVALID_TIME_RANGE);
}

#[test]
fn validate_trades_query_range_boundaries() {
    let day = MAX_TRADES_QUERY_RANGE_SECS;
    assert_eq!(
        validate_trades_query_range(Some(0), Some(0)).unwrap(),
        (0, 0)
    );
    assert_eq!(
        validate_trades_query_range(Some(10), Some(10 + day - 1)).unwrap(),
        (10, 10 + day - 1)
    );
    assert_eq!(
        validate_trades_query_range(Some(10), Some(10 + day)).unwrap(),
        (10, 10 + day)
    );

    let too_large = validate_trades_query_range(Some(10), Some(10 + day + 1)).unwrap_err();
    assert_eq!(too_large.code, TIME_RANGE_TOO_LARGE);
    assert!(too_large.message.contains("86401s"));

    for (start, end) in [
        (Some(11), Some(10)),
        (None, Some(10)),
        (Some(10), None),
        (None, None),
    ] {
        let error = validate_trades_query_range(start, end).unwrap_err();
        assert_eq!(
            error.code, INVALID_TIME_RANGE,
            "start={:?} end={:?}",
            start, end
        );
    }
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

    let env = envelope(
        EwdsOperation::FacilitiesQuery,
        "req-facilities-1",
        json!({}),
    );

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

// --- IdsQuery -------------------------------------------------------

#[tokio::test]
async fn ids_query_success() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    let env = envelope(
        EwdsOperation::IdsQuery,
        "req-ids-1",
        json!({ "offchainId": "offchain-abc" }),
    );

    handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap();

    let data = captured_data(&server).await;
    // get_or_create returns exactly one mapping, sent as vec![data]
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0]["onchain_id"],
        json!(bytes16_to_hex(create_encrypted_bytes16_from_string(
            "offchain-abc"
        )))
    );

    stop_app(app).await;
}

#[tokio::test]
async fn ids_query_get_or_create_is_idempotent() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // First request creates the mapping.
    handle_request(
        &app.db_wrapper,
        &client,
        &config,
        envelope(
            EwdsOperation::IdsQuery,
            "req-ids-create",
            json!({ "offchainId": "offchain-idem" }),
        ),
    )
    .await
    .unwrap();
    let first = captured_data(&server).await;
    assert_eq!(first.len(), 1);
    assert!(
        !first[0]["onchain_id"].is_null(),
        "expected an onchain_id in the first response"
    );
    let first_onchain = first[0]["onchain_id"].clone();

    // A fresh server, so received_requests() counts only the second call.
    let server2 = mock_gateway().await;
    let config2 = test_config(server2.uri());

    // Second request for the same offchain id returns the same mapping.
    handle_request(
        &app.db_wrapper,
        &client,
        &config2,
        envelope(
            EwdsOperation::IdsQuery,
            "req-ids-again",
            json!({ "offchainId": "offchain-idem" }),
        ),
    )
    .await
    .unwrap();
    let second = captured_data(&server2).await;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0]["onchain_id"], first_onchain);

    stop_app(app).await;
}

#[tokio::test]
async fn ids_query_bad_payload_errors() {
    let app = init_app().await;
    let server = mock_gateway().await;
    let config = test_config(server.uri());
    let client = reqwest::Client::new();

    // offchain_id is required -> missing field errors
    let env = envelope(EwdsOperation::IdsQuery, "req-ids-bad", json!({}));

    let err = handle_request(&app.db_wrapper, &client, &config, env)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("id.query payload parse error"));
    assert!(server.received_requests().await.unwrap().is_empty());

    stop_app(app).await;
}
