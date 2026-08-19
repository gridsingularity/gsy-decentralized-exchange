use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::ewds_handler::{handle_request, EwdsHandlerConfig};
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
