use primitives::db_api_schema::grid_topology::FacilityMarketValidationRequest;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

// Deliberately different from locally hashed IDs: callers must use the ID service's result.
pub const FACILITY_1_ONCHAIN: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const FACILITY_2_ONCHAIN: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub async fn setup_facilities_mock(facility_ids: [&str; 2]) -> MockServer {
    let server = MockServer::start().await;
    let known_ids = facility_ids.map(str::to_string);
    Mock::given(method("POST"))
        .and(path("/facilities/validate"))
        .respond_with(move |request: &Request| {
            let pairs: Vec<FacilityMarketValidationRequest> =
                serde_json::from_slice(&request.body).unwrap();
            ResponseTemplate::new(200).set_body_json(
                pairs
                    .into_iter()
                    .map(|pair| {
                        let eligible = known_ids.contains(&pair.facility_id);
                        json!({
                            "facility_id": pair.facility_id, "market_id": pair.market_id,
                            "eligible": eligible,
                            "reason": if eligible { None } else { Some("unknown_facility") }
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .with_priority(10)
        .mount(&server)
        .await;
    for (facility_id, onchain_id) in facility_ids
        .into_iter()
        .zip([FACILITY_1_ONCHAIN, FACILITY_2_ONCHAIN])
    {
        Mock::given(method("POST"))
            .and(path("/ids"))
            .and(query_param("offchain_id", facility_id))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "offchain_id": facility_id, "onchain_id": onchain_id, "creation_time": 0
            })))
            .mount(&server)
            .await;
    }
    std::env::set_var("OFFCHAIN_STORAGE_URL", server.uri());
    std::env::set_var("OFFCHAIN_STORAGE_TRANSPORT", "http");
    std::env::set_var("EWDS_ENABLE_HANDLER", "false");
    server
}
