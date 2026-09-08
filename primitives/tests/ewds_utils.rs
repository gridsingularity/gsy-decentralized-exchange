use primitives::db_api_schema::grid_topology::FacilitySchema;
use primitives::ewds::utils::fetch_facility_owner_mapping;
use std::env;
use std::sync::Mutex;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

#[tokio::test]
async fn fetches_mapping_over_http() {
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

    env::remove_var("OFFCHAIN_STORAGE_TRANSPORT");
    env::set_var("OFFCHAIN_STORAGE_URL", server.uri());

    let mapping = fetch_facility_owner_mapping("UNUSED_ENV", "unused-default")
        .await
        .unwrap();

    assert_eq!(mapping.len(), 2);
    assert_eq!(
        mapping.get("AIS1-House-1").map(String::as_str),
        Some("owner 1")
    );
    assert_eq!(
        mapping.get("AIS1-House-2").map(String::as_str),
        Some("owner 2")
    );

    env::remove_var("OFFCHAIN_STORAGE_URL");
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

    env::remove_var("OFFCHAIN_STORAGE_TRANSPORT");
    env::set_var("OFFCHAIN_STORAGE_URL", server.uri());

    let mapping = fetch_facility_owner_mapping("UNUSED_ENV", "unused-default")
        .await
        .unwrap();

    assert!(mapping.is_empty());

    env::remove_var("OFFCHAIN_STORAGE_URL");
}

#[tokio::test]
async fn errors_on_non_success_status() {
    let _guard = ENV_LOCK.lock().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/facilities"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    env::remove_var("OFFCHAIN_STORAGE_TRANSPORT");
    env::set_var("OFFCHAIN_STORAGE_URL", server.uri());

    let err = fetch_facility_owner_mapping("UNUSED_ENV", "unused-default")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Failed to fetch facilities"));

    env::remove_var("OFFCHAIN_STORAGE_URL");
}
