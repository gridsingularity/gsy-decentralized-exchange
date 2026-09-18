use primitives::db_api_schema::orders::{
    order_metadata_to_contract, DbAttributes, DbRequirements, EnergyType,
};
use primitives::utils::endpoint_calls::resolve_order_partner_ids;
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
    json!({"offchain_id": offchain_id, "onchain_id": onchain_id, "creation_time": 1})
}

// Keep both transports in one test so environment overrides cannot race.
#[tokio::test]
async fn resolves_partner_facility_ids_over_http_and_ewds() {
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
        let mut requirements = Some(DbRequirements {
            trading_partner_id: Some(FACILITY_UUID.to_string()),
            energy_type: Some(EnergyType::Green),
            preferred_energy_rate: Some(11.0),
        });
        let mut attributes = Some(DbAttributes {
            trading_partner_id: Some(HEX_FACILITY_ID.to_string()),
            energy_type: EnergyType::Pv,
        });
        resolve_order_partner_ids(
            &mut requirements,
            &mut attributes,
            "TEST_IDS_CLIENT_ID",
            "testids",
        )
        .await
        .unwrap();
        let encoded =
            order_metadata_to_contract(requirements.as_ref(), attributes.as_ref()).unwrap();
        assert_eq!(encoded.preferred_trading_partner, [0x11; 16]);
        assert_eq!(encoded.trading_partner, [0x22; 16]);
        assert_eq!(encoded.preferred_energy_rate, 110_000);
        assert_eq!(encoded.energy_source_preference, 1);
        assert_eq!(encoded.energy_type, 2);

        let count = server.received_requests().await.unwrap().len();
        resolve_order_partner_ids(&mut None, &mut None, "TEST_IDS_CLIENT_ID", "testids")
            .await
            .unwrap();
        assert_eq!(server.received_requests().await.unwrap().len(), count);

        requirements.as_mut().unwrap().trading_partner_id = Some("invalid-mapping".to_string());
        assert!(resolve_order_partner_ids(
            &mut requirements,
            &mut None,
            "TEST_IDS_CLIENT_ID",
            "testids"
        )
        .await
        .is_err());
    }
}
