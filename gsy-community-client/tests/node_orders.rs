#[path = "common/facilities.rs"]
mod facilities;

use facilities::{FACILITY_1_ONCHAIN, FACILITY_2_ONCHAIN};
use gsy_community_client::node_connector::orders::create_input_orders;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use primitives::db_api_schema::market::MarketSchema;
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::EwdsSendMessageDto;
use primitives::utils::{parse_uuid_or_hex_bytes16, NODE_FLOAT_SCALING_FACTOR};
use primitives::{MarketType, MatchingAlgorithm};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

async fn setup_facilities_mock() -> MockServer {
    facilities::setup_facilities_mock(["area1", "area2"]).await
}

fn test_market() -> MarketSchema {
    MarketSchema {
        market_id: format!("0x{}", "11".repeat(16)),
        community_id: "community-1".to_string(),
        opening_time: "00000000000000456446".to_string(),
        closing_time: "00000000000000456447".to_string(),
        delivery_start_time: "00000000000000456456".to_string(),
        delivery_end_time: "00000000000000456457".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: "00000000000000345345".to_string(),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_orders_to_evm_params_are_created_correctly() {
    let _facilities_server = setup_facilities_mock().await;

    let forecasts: Vec<ForecastSchema> = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 123_123,
            time_slot: 456_456,
            energy_kwh: 12.0,
            community_uuid: "community1".to_string(),
            confidence: 0.8,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 234_234,
            time_slot: 456_456,
            energy_kwh: -1.0,
            community_uuid: "community1".to_string(),
            confidence: 0.1,
        },
    ];

    let market = test_market();
    let input_orders = create_input_orders(forecasts, market.clone())
        .await
        .expect("create_input_orders failed");
    assert_eq!(input_orders.len(), 2);

    let current_time = get_current_timestamp_in_secs();

    let (
        _bid_order_id,
        bid_created_by,
        bid_market,
        bid_slot,
        bid_creation,
        bid_energy,
        bid_rate,
        _bid_preferences,
        _bid_attributes,
        bid_type,
        bid_preferred_partner,
        bid_preferred_rate,
    ) = input_orders[0];
    assert_eq!(
        bid_created_by,
        parse_uuid_or_hex_bytes16(FACILITY_1_ONCHAIN).unwrap()
    );
    assert_eq!(
        bid_market,
        parse_uuid_or_hex_bytes16(market.market_id.as_str()).expect("failed to parse market id")
    );
    // delivery_start_time from market drives the slot now, not forecast.time_slot
    assert_eq!(bid_slot, 456_456);
    assert!(current_time >= bid_creation && current_time - bid_creation <= 1);
    assert_eq!(bid_energy, (12.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(bid_rate, (12.0 * 0.3 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(bid_type);
    assert_eq!(bid_preferred_partner, [0; 16]);
    assert_eq!(bid_preferred_rate, 0);

    let (
        _offer_order_id,
        offer_created_by,
        offer_market,
        offer_slot,
        offer_creation,
        offer_energy,
        offer_rate,
        _offer_preferences,
        _offer_attributes,
        offer_type,
        offer_preferred_partner,
        offer_preferred_rate,
    ) = input_orders[1];
    assert_eq!(
        offer_created_by,
        parse_uuid_or_hex_bytes16(FACILITY_2_ONCHAIN).unwrap()
    );
    assert_eq!(
        offer_market,
        parse_uuid_or_hex_bytes16(market.market_id.as_str()).expect("failed to parse market id")
    );
    assert_eq!(offer_slot, 456_456);
    assert!(current_time >= offer_creation && current_time - offer_creation <= 1);
    assert_eq!(offer_energy, (1.0 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert_eq!(offer_rate, (1.0 * 0.07 * NODE_FLOAT_SCALING_FACTOR) as u64);
    assert!(!offer_type);
    assert_eq!(offer_preferred_partner, [0; 16]);
    assert_eq!(offer_preferred_rate, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn test_create_input_orders_skips_if_facility_does_not_exist() {
    let _facilities_server = setup_facilities_mock().await;

    let market = test_market();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 5.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "missing-facility".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 7.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market)
        .await
        .expect("create_input_orders failed");
    assert_eq!(orders.len(), 1);
}

#[tokio::test]
#[serial_test::serial]
async fn test_create_input_orders_skips_zero_energy_forecasts() {
    let _facilities_server = setup_facilities_mock().await;

    let market = test_market();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 0.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: -2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market)
        .await
        .expect("create_input_orders failed");
    assert_eq!(orders.len(), 1);
    assert!(!orders[0].9);
}

#[tokio::test]
#[serial_test::serial]
async fn test_create_input_orders_assigns_unique_order_ids_and_stable_side_mapping() {
    let _facilities_server = setup_facilities_mock().await;

    let market = test_market();
    let forecasts = vec![
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 2.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area2".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: -3.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
        ForecastSchema {
            facility_id: "area1".to_string(),
            creation_time: 1,
            time_slot: 456_456,
            energy_kwh: 4.0,
            community_uuid: "community1".to_string(),
            confidence: 0.9,
        },
    ];

    let orders = create_input_orders(forecasts, market)
        .await
        .expect("create_input_orders failed");
    assert_eq!(orders.len(), 3);

    assert!(orders[0].9);
    assert!(!orders[1].9);
    assert!(orders[2].9);

    let order_ids: HashSet<[u8; 16]> = orders.iter().map(|order| order.0).collect();
    assert_eq!(order_ids.len(), orders.len());
}

fn forecast(facility_id: &str, energy_kwh: f64) -> ForecastSchema {
    ForecastSchema {
        facility_id: facility_id.to_string(),
        creation_time: 1,
        time_slot: 456_456,
        energy_kwh,
        community_uuid: "community-1".to_string(),
        confidence: 0.9,
    }
}

#[tokio::test]
#[serial_test::serial]
async fn empty_or_zero_forecasts_do_not_query_storage() {
    let server = setup_facilities_mock().await;
    for forecasts in [vec![], vec![forecast("area1", 0.0)]] {
        assert!(create_input_orders(forecasts, test_market())
            .await
            .unwrap()
            .is_empty());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn ineligible_forecasts_do_not_resolve_ids_or_contact_ewds_in_http_mode() {
    let server = setup_facilities_mock().await;
    for reason in [
        "unknown_market",
        "unknown_facility",
        "unknown_community",
        "unknown_site",
        "facility_not_listed_in_site",
        "site_not_in_community",
    ] {
        Mock::given(method("POST"))
            .and(path("/facilities/validate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "facility_id": "area1", "market_id": test_market().market_id,
                "eligible": false, "reason": reason,
            }])))
            .with_priority(1)
            .up_to_n_times(1)
            .mount(&server)
            .await;
        assert!(
            create_input_orders(vec![forecast("area1", 5.0)], test_market())
                .await
                .unwrap()
                .is_empty(),
            "{reason}"
        );
    }
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 6);
    assert!(requests
        .iter()
        .all(|request| request.url.path() == "/facilities/validate"));
}

#[tokio::test]
#[serial_test::serial]
async fn validation_errors_fail_before_id_resolution_and_can_be_retried() {
    let server = setup_facilities_mock().await;
    let previous_gateway = std::env::var("EWDS_GATEWAY_URL").ok();
    std::env::set_var("EWDS_GATEWAY_URL", server.uri());
    std::env::set_var("EWDS_ENABLE_HANDLER", "true");
    for response in [
        ResponseTemplate::new(500),
        ResponseTemplate::new(200).set_body_string("not JSON"),
        ResponseTemplate::new(200).set_body_json(json!([])),
        ResponseTemplate::new(200).set_body_json(json!([{
            "facility_id": "another-facility", "market_id": test_market().market_id,
            "eligible": true,
        }])),
    ] {
        Mock::given(method("POST"))
            .and(path("/facilities/validate"))
            .respond_with(response)
            .with_priority(1)
            .up_to_n_times(1)
            .mount(&server)
            .await;
        let error = create_input_orders(vec![forecast("area1", 5.0)], test_market())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("Facility participation validation failed"));
    }
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|request| request.url.path() == "/facilities/validate"));
    let orders = create_input_orders(vec![forecast("area1", 5.0)], test_market())
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(
        orders[0].1,
        parse_uuid_or_hex_bytes16(FACILITY_1_ONCHAIN).unwrap()
    );
    std::env::set_var("EWDS_ENABLE_HANDLER", "false");
    match previous_gateway {
        Some(url) => std::env::set_var("EWDS_GATEWAY_URL", url),
        None => std::env::remove_var("EWDS_GATEWAY_URL"),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn forecast_validation_and_rejection_notifications_work_over_ewds() {
    let gateway = setup_facilities_mock().await;
    let pending = Arc::new(Mutex::new(Value::Null));
    let response = pending.clone();
    Mock::given(method("POST"))
        .and(path("/api/v2/messages"))
        .respond_with(move |request: &Request| {
            let message: EwdsSendMessageDto = serde_json::from_slice(&request.body).unwrap();
            let payload: Value = serde_json::from_str(&message.payload).unwrap();
            if let Some(operation) = payload["operation"].as_str() {
                let data = match operation {
                    "facilities.validate" => {
                        assert_eq!(payload["payload"], json!([
                            {"facility_id": "missing-facility", "market_id": test_market().market_id},
                            {"facility_id": "area1", "market_id": test_market().market_id},
                            {"facility_id": "area2", "market_id": test_market().market_id},
                        ]));
                        json!([
                            {"facility_id": "missing-facility", "market_id": test_market().market_id,
                                "eligible": false, "reason": "unknown_facility"},
                            {"facility_id": "area1", "market_id": test_market().market_id,
                                "eligible": false, "reason": "site_not_in_community"},
                            {"facility_id": "area2", "market_id": test_market().market_id, "eligible": true},
                        ])
                    }
                    "ids.query" => {
                        assert_eq!(payload["payload"]["offchain_id"], "area2");
                        json!([{"offchain_id": "area2", "onchain_id": FACILITY_2_ONCHAIN, "creation_time": 0}])
                    }
                    other => panic!("unexpected query {other}"),
                };
                *response.lock().unwrap() = json!({"requestId": payload["requestId"], "success": true, "data": data});
            }
            ResponseTemplate::new(200).set_body_json(json!({"recipients": {"sent": 1, "failed": 0, "total": 1}}))
        }).mount(&gateway).await;
    Mock::given(method("GET"))
        .and(path("/api/v2/messages"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!([
                {"payload": pending.lock().unwrap().to_string()}
            ]))
        })
        .mount(&gateway)
        .await;
    let previous_gateway = std::env::var("EWDS_GATEWAY_URL").ok();
    std::env::set_var("EWDS_GATEWAY_URL", gateway.uri());
    std::env::set_var("OFFCHAIN_STORAGE_TRANSPORT", "ewds");
    std::env::set_var("EWDS_ENABLE_HANDLER", "true");
    let result = create_input_orders(
        vec![
            forecast("missing-facility", 5.0),
            forecast("area1", -2.0),
            forecast("area2", -3.0),
        ],
        test_market(),
    )
    .await;
    std::env::set_var("OFFCHAIN_STORAGE_TRANSPORT", "http");
    std::env::set_var("EWDS_ENABLE_HANDLER", "false");
    match previous_gateway {
        Some(url) => std::env::set_var("EWDS_GATEWAY_URL", url),
        None => std::env::remove_var("EWDS_GATEWAY_URL"),
    }
    let orders = result.unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(
        orders[0].1,
        parse_uuid_or_hex_bytes16(FACILITY_2_ONCHAIN).unwrap()
    );
    let requests = gateway.received_requests().await.unwrap();
    assert!(requests
        .iter()
        .all(|request| request.url.path() == "/api/v2/messages"));
    let notifications: Vec<Value> = requests
        .iter()
        .filter(|request| request.method == "POST")
        .map(|request| serde_json::from_slice::<EwdsSendMessageDto>(&request.body).unwrap())
        .map(|message| serde_json::from_str::<Value>(&message.payload).unwrap())
        .filter(|payload| payload["eventType"] == "orderStatus.event")
        .collect();
    assert_eq!(notifications.len(), 2);
    for (notification, facility_id, order_type, energy, rate, reason) in [
        (
            &notifications[0],
            "missing-facility",
            "bid",
            5.0,
            1.5,
            "UnknownFacility",
        ),
        (
            &notifications[1],
            "area1",
            "offer",
            2.0,
            0.14,
            "SiteNotInCommunity",
        ),
    ] {
        let order = &notification["data"][0];
        assert_eq!(order["createdBy"], facility_id);
        assert_eq!(order["marketId"], test_market().market_id);
        assert_eq!(order["orderStatus"], "rejected");
        assert_eq!(order["orderType"], order_type);
        assert_eq!(order["quantity"], energy);
        assert_eq!(order["priceLimit"], rate);
        assert!(order["rejectReason"].as_str().unwrap().contains(reason));
        assert!(order["updatedAt"].is_string());
        uuid::Uuid::parse_str(order["orderId"].as_str().unwrap()).unwrap();
    }
}
