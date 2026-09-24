use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::ewds::dto::EwdsCommunityDto;
use primitives::offchain_storage::{
    CommunityProvider, OffchainStorageClient, OffchainStorageTransport,
};
use serde_json::{json, Value};
use std::env;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{method, path};
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
