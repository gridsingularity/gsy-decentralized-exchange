use primitives::offchain_storage::OffchainStorageClient;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const FACILITY_UUID: &str = "00112233-4455-6677-8899-aabbccddeeff";
const HEX_FACILITY_ID: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn mapping(offchain_id: &str) -> Value {
    let onchain_id = match offchain_id {
        FACILITY_UUID => "0x11111111111111111111111111111111",
        HEX_FACILITY_ID => "0x22222222222222222222222222222222",
        _ => "not-an-onchain-id",
    };
    json!({
        "offchain_id": offchain_id,
        "onchain_id": onchain_id,
        "creation_time": "1970-01-01T00:00:01+00:00",
    })
}

// Keep both transports in one test so environment overrides cannot race.
#[tokio::test]
async fn legacy_id_lookup_preserves_opaque_ids_over_http_and_ewds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ids"))
        .respond_with(|request: &Request| {
            let offchain_id = request
                .url
                .query_pairs()
                .find(|(key, _)| key == "offchain_id")
                .unwrap()
                .1;
            ResponseTemplate::new(200).set_body_json(mapping(&offchain_id))
        })
        .mount(&server)
        .await;

    let pending = Arc::new(Mutex::new(Value::Null));
    let sent = pending.clone();
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let envelope: Value = serde_json::from_str(body["payload"].as_str().unwrap()).unwrap();
            assert_eq!(envelope["operation"], "ids.query");
            let offchain_id = envelope["payload"]["offchain_id"].as_str().unwrap();
            *sent.lock().unwrap() = json!({
                "requestId": envelope["requestId"], "success": true,
                "data": [mapping(offchain_id)]
            });
            ResponseTemplate::new(200).set_body_json(json!({
                "recipients": {"sent": 1, "failed": 0, "total": 1}
            }))
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!([
                {"payload": pending.lock().unwrap().to_string()}
            ]))
        })
        .mount(&server)
        .await;

    std::env::set_var("OFFCHAIN_STORAGE_URL", server.uri());
    std::env::set_var("EWDS_GATEWAY_URL", server.uri());
    std::env::set_var("EWDS_RESPONSE_TIMEOUT_MS", "1000");
    for transport in ["http", "ewds"] {
        std::env::set_var("OFFCHAIN_STORAGE_TRANSPORT", transport);
        let id_mapping_source = OffchainStorageClient::from_env("TEST_IDS_CLIENT_ID", "testids");
        assert_eq!(
            id_mapping_source
                .fetch_onchain_id(FACILITY_UUID)
                .await
                .unwrap(),
            "0x11111111111111111111111111111111"
        );
        // Hex-shaped strings remain off-chain identities, not pre-resolved hashes.
        assert_eq!(
            id_mapping_source
                .fetch_onchain_id(HEX_FACILITY_ID)
                .await
                .unwrap(),
            "0x22222222222222222222222222222222"
        );
        assert!(id_mapping_source
            .fetch_onchain_id("invalid-mapping")
            .await
            .is_err());
    }
}
