//! MS-11: the JSON body limit of `POST /match` (`json_body_limit`). Dry runs
//! against an unreachable database: no MongoDB needed.

mod common;

use actix_web::dev::ServiceResponse;
use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use common::payload::{BatchShape, batch_of_size};
use launchpad_matching_service::api::types::DbBidOfferMatch;
use launchpad_matching_service::api::views;
use launchpad_matching_service::auth::jwt::{JwtSecret, encode_jwt};
use launchpad_matching_service::configuration::DEFAULT_JSON_BODY_LIMIT;
use std::collections::HashMap;

const TEST_JWT_SECRET: &str = "test_secret_for_unit_tests";
const MIB: usize = 1024 * 1024;

fn bearer_token() -> String {
    let token = encode_jwt("test_user", TEST_JWT_SECRET, 1).expect("Failed to encode JWT");
    format!("Bearer {}", token)
}

async fn unreachable_database() -> mongodb::Database {
    let client = mongodb::Client::with_uri_str(
        "mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=200&connectTimeoutMS=200",
    )
    .await
    .expect("Failed to parse the MongoDB URI");
    client.database("launchpad_matching_service_unreachable")
}

/// Test service with `POST /match` and a JSON body limit of `$limit` bytes.
macro_rules! match_app {
    ($limit:expr) => {
        test::init_service(
            App::new()
                .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
                .app_data(web::Data::new(unreachable_database().await))
                .app_data(views::json_config($limit))
                .app_data(views::query_config())
                .service(views::pay_as_bid),
        )
        .await
    };
}

fn dry_run_request(body: Vec<u8>) -> test::TestRequest {
    test::TestRequest::post()
        .uri("/match?dry_run=true")
        .insert_header(("Authorization", bearer_token()))
        .insert_header(("Content-Type", "application/json"))
        .set_payload(body)
}

async fn assert_payload_too_large(resp: ServiceResponse) {
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "payload_too_large");
    assert!(
        !body["message"].as_str().unwrap_or("").is_empty(),
        "body {}",
        body
    );
}

#[actix_web::test]
async fn test_body_over_configured_limit_is_413_json() {
    let limit = 4 * 1024;
    let app = match_app!(limit);

    // Largest valid batch under the limit: accepted.
    let (batch, body) = batch_of_size(BatchShape::Realistic, limit, 1);
    assert!(body.len() <= limit && body.len() > limit - 1024);
    let resp = test::call_service(&app, dry_run_request(body).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let matches: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    assert!(!batch.orders.is_empty());
    assert!(!matches.is_empty());

    // A body of exactly the limit is accepted (padding after the JSON
    // value is whitespace).
    let (_, mut body) = batch_of_size(BatchShape::Realistic, limit, 1);
    body.resize(limit, b' ');
    let resp = test::call_service(&app, dry_run_request(body).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // One byte over: 413 with a JSON body, before the JSON is parsed.
    let (_, mut body) = batch_of_size(BatchShape::Realistic, limit, 1);
    body.resize(limit + 1, b' ');
    let resp = test::call_service(&app, dry_run_request(body).to_request()).await;
    assert_payload_too_large(resp).await;
}

#[actix_web::test]
async fn test_default_limit_accepts_10_mib_and_rejects_over_16_mib() {
    let app = match_app!(DEFAULT_JSON_BODY_LIMIT);

    // Plan max of the Advanced plan (10 MiB, rejected by actix-web's 2 MiB
    // default before MS-11): accepted.
    let (batch, body) = batch_of_size(BatchShape::Realistic, 10 * MIB, 2);
    assert!(body.len() > 10 * MIB - 1024);
    let resp = test::call_service(&app, dry_run_request(body).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let matches: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    assert!(batch.orders.len() > 10_000);
    assert!(matches.values().map(Vec::len).sum::<usize>() > 1_000);

    // Over the default 16 MiB: 413 JSON.
    let body = vec![b' '; DEFAULT_JSON_BODY_LIMIT + 1];
    let resp = test::call_service(&app, dry_run_request(body).to_request()).await;
    assert_payload_too_large(resp).await;
}
