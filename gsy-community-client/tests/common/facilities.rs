use primitives::ewds::dto::FacilityMarketValidationRequest;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

pub async fn setup_facilities_mock(facility_ids: [&str; 2]) -> MockServer {
    let server = MockServer::start().await;
    let known_ids = facility_ids.map(str::to_owned);
    Mock::given(method("GET"))
        .and(path("/facilities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            facility_ids.iter().enumerate().map(|(index, facility_id)| json!({
                "facility_id": facility_id, "facility_name": facility_id,
                "site_id": "site-1", "owner_id": format!("owner-{}", index + 1)
            })).collect::<Vec<_>>(),
        ))
        .mount(&server)
        .await;
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
    Mock::given(method("POST"))
        .and(path("/ids"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    std::env::set_var("OFFCHAIN_STORAGE_URL", server.uri());
    std::env::set_var("OFFCHAIN_STORAGE_TRANSPORT", "http");
    std::env::set_var("EWDS_ENABLE_HANDLER", "false");
    server
}
