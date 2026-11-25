use chrono::DateTime;
use primitives::ewds::dto::EwdsCommunityDto;
use crate::helpers::{init_app, stop_app, ts};
use gsy_offchain_storage::db::DatabaseWrapper;
use mongodb::{bson::doc, Client};
use primitives::db_api_schema::grid_topology::{
    EnergyCommunitySchema, FacilitySchema, SiteSchema,
};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::ewds::dto::{
    EwdsFacilityDto, EwdsRequestEnvelope, EwdsSendMessageDto, EwdsSiteDto,
    FacilityMarketValidationReason, FacilityMarketValidationRequest,
};
use primitives::ewds::EwdsOperation;
use serde_json::{json, Value};

async fn validate_via_ewds(db: &DatabaseWrapper, payload: Value) -> Value {
    let gateway = crate::ewds_handler::mock_gateway().await;
    let config = crate::ewds_handler::test_config(gateway.uri());
    gsy_offchain_storage::ewds_handler::handle_request(
        db,
        &reqwest::Client::new(),
        &config,
        EwdsRequestEnvelope {
            request_id: "validation-test".to_string(),
            operation: EwdsOperation::FacilitiesValidate,
            payload,
        },
    )
    .await
    .unwrap();
    let sent = gateway.received_requests().await.unwrap();
    assert_eq!(sent.len(), 1);
    let message: EwdsSendMessageDto = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!(
        message.topic_name,
        config
            .topics
            .for_operation(EwdsOperation::FacilitiesValidate)
            .response
    );
    let response: Value = serde_json::from_str(&message.payload).unwrap();
    assert_eq!(response["requestId"], "validation-test");
    response
}

fn pair(facility_id: &str, market_id: &str) -> FacilityMarketValidationRequest {
    FacilityMarketValidationRequest {
        facility_id: facility_id.to_string(),
        market_id: market_id.to_string(),
    }
}

fn community_id(id: &str) -> String {
    format!("Community-{id}")
}

fn market_id(id: &str) -> String {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static IDS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    IDS.get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .entry(id.to_string())
        .or_insert_with(|| uuid::Uuid::new_v4().to_string())
        .clone()
}

async fn seed_community(db: &DatabaseWrapper, id: &str) {
    db.markets()
        .insert_one(MarketSchema {
            market_id: market_id(id),
            community_id: community_id(id),
            opening_time: ts(100),
            closing_time: ts(200),
            delivery_start_time: ts(300),
            delivery_end_time: ts(400),
            market_type: MarketType::Spot,
            matching_algorithm: MatchingAlgorithm::PayAsBid,
            created_at: ts(100),
        })
        .await
        .unwrap();
    db.communities()
        .store(vec![community(id, vec![format!("site-{id}")])])
        .await
        .unwrap();
    db.sites()
        .store(vec![site(id, vec![format!("facility-{id}")])])
        .await
        .unwrap();
    db.facilities()
        .store(vec![facility(
            &format!("facility-{id}"),
            &format!("site-{id}"),
        )])
        .await
        .unwrap();
}

#[tokio::test]
async fn facility_ingress_preserves_ids_and_validation_echoes_each_request() {
    use gsy_offchain_storage::ewds_event_handler::handle_event;
    use primitives::ewds::{dto::EwdsEventEnvelope, EwdsEventType};

    let facility_id = "abcdefab-abcd-abcd-abcd-abcdefabcdef";
    let owner_id = "fedcbafe-dcba-dcba-dcba-fedcbafedcba";
    for ewds in [false, true] {
        let app = init_app().await;
        seed_community(&app.db_wrapper, "a").await;
        let client = reqwest::Client::new();
        for id in [facility_id.to_uppercase(), facility_id.to_string()] {
            let facility = json!({"facility_id": id, "owner_id": owner_id.to_uppercase(),
                "facility_name": format!("UUID facility {id}"), "site_id": "site-a"});
            let site = json!({"site_id": "site-a", "site_name": "Renamed display name", "site_description": "Unchanged description",
                "facilities": ["facility-a", facility_id.to_uppercase(), facility_id]});
            for (route, event_type, data) in [
                ("facilities", EwdsEventType::Facility, facility),
                ("sites", EwdsEventType::Site, site),
            ] {
                if ewds {
                    handle_event(
                        &app.db_wrapper,
                        EwdsEventEnvelope {
                            event_id: "topology-test".into(),
                            event_type,
                            occurred_at: "2026-11-04T10:00:00Z".into(),
                            data: json!([data]),
                        },
                    )
                    .await
                    .unwrap();
                } else {
                    let response = client
                        .post(format!("{}/{route}", app.address))
                        .json(&data)
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status(), reqwest::StatusCode::OK);
                }
            }
        }
        let facility = app
            .db_wrapper
            .facilities()
            .find_one(doc! {"facility_id": facility_id})
            .await
            .unwrap()
            .unwrap();
        assert_eq!(facility.owner_id, owner_id.to_uppercase());
        assert_eq!(
            app.db_wrapper
                .facilities()
                .count_documents(doc! {})
                .await
                .unwrap(),
            3
        );
        let site = app.db_wrapper.sites().get_by_ids(&["site-a"]).await.unwrap().remove(0);
        assert_eq!(site.facilities, vec!["facility-a".to_string(), facility_id.to_uppercase(), facility_id.to_string()]);
        let requests = [
            pair(&facility_id.to_uppercase(), &market_id("a")),
            pair(" padded ", &market_id("a")),
            pair(facility_id, &market_id("a")),
        ];
        let response: Value = client
            .post(format!("{}/facilities/validate", app.address))
            .json(&requests)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let ewds_response = validate_via_ewds(&app.db_wrapper, json!(requests)).await;
        assert_eq!(response, ewds_response["data"]);
        assert_eq!(response.as_array().unwrap().len(), requests.len());
        for (i, request) in requests.iter().enumerate() {
            assert_eq!(response[i]["facility_id"], request.facility_id);
            assert_eq!(response[i]["market_id"], request.market_id);
            assert_eq!(response[i]["eligible"], i != 1);
        }
        assert_eq!(response[1]["reason"], "unknown_facility");
        for (route, event_type, data) in [
            (
                "facilities",
                EwdsEventType::Facility,
                json!({"facility_id": "bad ",
                "owner_id": owner_id, "facility_name": "Invalid", "site_id": "site-a"}),
            ),
            (
                "facilities",
                EwdsEventType::Facility,
                json!({"facility_id": "valid",
                "owner_id": "", "facility_name": "Invalid", "site_id": "site-a"}),
            ),
            (
                "sites",
                EwdsEventType::Site,
                json!({"site_id": "site-a", "site_name": "site-a",
                "site_description": "Invalid", "facilities": [" "]}),
            ),
        ] {
            assert_eq!(
                client
                    .post(format!("{}/{route}", app.address))
                    .json(&data)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                reqwest::StatusCode::BAD_REQUEST
            );
            let error = handle_event(
                &app.db_wrapper,
                EwdsEventEnvelope {
                    event_id: "invalid-test".into(),
                    event_type,
                    occurred_at: "2026-11-04T10:00:00Z".into(),
                    data: json!([data]),
                },
            )
            .await
            .unwrap_err();
            assert!(primitives::ewds::is_invalid_event(&error));
        }
        stop_app(app).await;
    }
}

#[tokio::test]
async fn http_community_ids_are_exact_and_eligibility_requires_the_same_identity() {
    let app = init_app().await;
    seed_community(&app.db_wrapper, "a").await;
    let communities = [
        "COMMUNITY-A",
        "Community-MiXeD/LIC01",
        "ABCDEFAB-1234-1234-1234-ABCDEFABCDEF",
        "abcdefab-1234-1234-1234-abcdefabcdef",
        "0xabcdefab123412341234abcdefabcdef",
    ];
    for id in communities {
        app.db_wrapper
            .markets()
            .update_one(
                doc! {"market_id": market_id("a")},
                doc! {"$set": {"community_id": id}},
            )
            .await
            .unwrap();
        let result = validate_via_ewds(
            &app.db_wrapper,
            json!([pair("facility-a", &market_id("a"))]),
        )
        .await;
        assert_eq!(result["data"][0]["eligible"], false);
        assert_eq!(result["data"][0]["reason"], "unknown_community");
        let response = reqwest::Client::new()
            .post(format!("{}/communities", app.address))
            .json(&EwdsCommunityDto::from(EnergyCommunitySchema {
                valid_from: DateTime::UNIX_EPOCH,
                valid_until: None,
                community_id: id.into(),
                community_name: format!("Community {id}"),
                sites: vec!["site-a".into()],
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response
                .json::<EwdsCommunityDto>()
                .await
                .unwrap()
                .community_id,
            id
        );
        let result = validate_via_ewds(
            &app.db_wrapper,
            json!([pair("facility-a", &market_id("a"))]),
        )
        .await;
        assert_eq!(result["data"][0]["eligible"], true);
    }
    assert_eq!(
        app.db_wrapper
            .communities()
            .count_documents(doc! {})
            .await
            .unwrap(),
        (communities.len() + 1) as u64
    );
    let response = reqwest::Client::new()
        .post(format!("{}/communities", app.address))
        .json(&json!({"communityId": " padded ", "communityName": "invalid", "sites": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    stop_app(app).await;
}

fn community(id: &str, sites: Vec<String>) -> EnergyCommunitySchema {
    EnergyCommunitySchema {
        community_id: community_id(id),
        community_name: id.to_string(),
        sites,
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
    }
}

fn site(id: &str, facilities: Vec<String>) -> SiteSchema {
    SiteSchema {
        site_id: format!("site-{id}"),
        site_name: format!("site-{id}"),
        site_description: String::new(),
        facilities,
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
    }
}

fn facility(facility_id: &str, site_id: &str) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: facility_id.to_string(),
        site_id: site_id.to_string(),
        owner_id: format!("owner-{}", facility_id.trim_start_matches("facility-")),
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
    }
}

#[tokio::test]
async fn http_topology_replacements_update_eligibility_and_preserve_name_conflicts() {
    let app = init_app().await;
    seed_community(&app.db_wrapper, "a").await;
    let client = reqwest::Client::new();
    let mut facility = FacilitySchema {
        facility_name: "New facility".into(),
        ..facility("facility-new", "site-a")
    };
    for owner in ["owner-new", "owner-updated"] {
        facility.owner_id = owner.into();
        let response = client
            .post(format!("{}/facilities", app.address))
            .json(&EwdsFacilityDto::from(facility.clone()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.json::<EwdsFacilityDto>().await.unwrap().owner_id,
            owner
        );
    }
    let requests = [
        pair("facility-new", &market_id("a")),
        pair("facility-a", &market_id("a")),
    ];
    assert!(
        !app.db_wrapper
            .validate_facility_markets(&requests)
            .await
            .unwrap()[0]
            .eligible
    );
    for (facilities, original_eligible) in [
        (vec!["facility-a".into(), "facility-new".into()], true),
        (vec!["facility-new".into()], false),
    ] {
        let response = client
            .post(format!("{}/sites", app.address))
            .json(&EwdsSiteDto::from(SiteSchema {
                site_description: "Updated".into(),
                ..site("a", facilities)
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let results = app
            .db_wrapper
            .validate_facility_markets(&requests)
            .await
            .unwrap();
        assert!(results[0].eligible);
        assert_eq!(results[1].eligible, original_eligible);
    }
    facility.facility_id = "conflicting-id".into();
    let response = client
        .post(format!("{}/facilities", app.address))
        .json(&EwdsFacilityDto::from(facility))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        app.db_wrapper
            .facilities()
            .count_documents(doc! {"facility_id": "conflicting-id"})
            .await
            .unwrap(),
        0
    );
    stop_app(app).await;
}

#[tokio::test]
async fn validates_mixed_pairs_in_input_order_without_writes() {
    use FacilityMarketValidationReason::*;
    let app = init_app().await;
    let db = &app.db_wrapper;
    seed_community(db, "a").await;
    seed_community(db, "b").await;
    // An existing ID mapping is not evidence that the facility exists.
    db.ids().get_or_create("missing".to_string()).await.unwrap();
    let mappings_before = db.ids().count_documents(doc! {}).await.unwrap();
    let requests = vec![
        pair("facility-a", &market_id("a")),
        pair("facility-a", &market_id("b")),
        pair("missing", &market_id("a")),
        pair("facility-b", "missing"),
        pair("facility-b", &market_id("b")),
        pair("owner-a", &market_id("a")),
        pair("facility-a", &market_id("a")),
        pair("facility-a", "00000000-0000-0000-0000-000000000000"),
        pair(
            "facility-a",
            &primitives::utils::bytes16_to_uuid_string(
                primitives::utils::uuid_v4_to_bytes16(&market_id("a")).unwrap(),
            )
            .to_uppercase(),
        ),
    ];
    let results = db.validate_facility_markets(&requests).await.unwrap();
    assert_eq!(results.len(), requests.len());
    for ((result, request), reason) in results.iter().zip(&requests).zip([
        None,
        Some(SiteNotInCommunity),
        Some(UnknownFacility),
        Some(UnknownMarket),
        None,
        Some(UnknownFacility),
        None,
        Some(UnknownMarket),
        None,
    ]) {
        assert_eq!(result.facility_id, request.facility_id);
        assert_eq!(result.market_id, request.market_id);
        assert_eq!(result.eligible, reason.is_none());
        assert_eq!(result.reason, reason);
    }
    assert!(db.validate_facility_markets(&[]).await.unwrap().is_empty());
    assert_eq!(db.ids().count_documents(doc! {}).await.unwrap(), mappings_before);
    assert_eq!(db.orders().count_documents(doc! {}).await.unwrap(), 0);
    stop_app(app).await;
}

#[tokio::test]
async fn requires_every_link_in_the_topology_chain() {
    use FacilityMarketValidationReason::*;
    let app = init_app().await;
    let db = &app.db_wrapper;
    let cases = [
        (
            "no-community",
            "communities",
            "community_id",
            "no-community",
            doc! {},
            UnknownCommunity,
        ),
        (
            "no-site",
            "sites",
            "site_id",
            "site-no-site",
            doc! {},
            UnknownSite,
        ),
        (
            "no-facility-link",
            "sites",
            "site_id",
            "site-no-facility-link",
            doc! {"$set": {"facilities": []}},
            FacilityNotListedInSite,
        ),
        (
            "no-site-link",
            "communities",
            "community_id",
            "no-site-link",
            doc! {"$set": {"sites": []}},
            SiteNotInCommunity,
        ),
        (
            "wrong-site",
            "facilities",
            "facility_id",
            "facility-wrong-site",
            doc! {"$set": {"site_id": "site-other"}},
            FacilityNotListedInSite,
        ),
    ];
    seed_community(db, "other").await;
    for (id, collection, key, value, update, reason) in cases {
        seed_community(db, id).await;
        let value = if key == "community_id" {
            community_id(value)
        } else {
            value.to_string()
        };
        let collection = db.collection::<mongodb::bson::Document>(collection);
        if update.is_empty() {
            collection.delete_one(doc! {key: value}).await.unwrap();
        } else {
            collection
                .update_one(doc! {key: value}, update)
                .await
                .unwrap();
        }
        let results = db
            .validate_facility_markets(&[pair(&format!("facility-{id}"), &market_id(id))])
            .await
            .unwrap();
        assert!(!results[0].eligible);
        assert_eq!(results[0].reason, Some(reason));
    }
    stop_app(app).await;
}

#[tokio::test]
async fn sees_topology_updates_between_calls() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let requests = [pair("facility-a", &market_id("a"))];
    assert!(!db.validate_facility_markets(&requests).await.unwrap()[0].eligible);
    seed_community(db, "a").await;
    assert!(db.validate_facility_markets(&requests).await.unwrap()[0].eligible);

    // Only the latest version of an entry counts.
    db.communities()
        .store(vec![community("a", vec![])])
        .await
        .unwrap();
    assert!(!db.validate_facility_markets(&requests).await.unwrap()[0].eligible);
    db.communities()
        .store(vec![community("a", vec!["site-a".into()])])
        .await
        .unwrap();
    assert!(db.validate_facility_markets(&requests).await.unwrap()[0].eligible);

    // A deleted facility is unknown.
    db.facilities()
        .store(vec![FacilitySchema {
            valid_until: Some(DateTime::UNIX_EPOCH),
            ..facility("facility-a", "site-a")
        }])
        .await
        .unwrap();
    let result = &db.validate_facility_markets(&requests).await.unwrap()[0];
    assert_eq!(
        result.reason,
        Some(FacilityMarketValidationReason::UnknownFacility)
    );
    stop_app(app).await;
}

#[tokio::test]
async fn malformed_database_records_are_errors_not_rejections() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let cases = [
        (
            "market",
            "markets",
            "market_id",
            "market-market",
            "community_id",
        ),
        (
            "facility",
            "facilities",
            "facility_id",
            "facility-facility",
            "site_id",
        ),
        ("site", "sites", "site_id", "site-site", "facilities"),
        (
            "community",
            "communities",
            "community_id",
            "community",
            "sites",
        ),
    ];
    // Seeded before any record is broken: storing reads all latest entries.
    for (id, ..) in cases {
        seed_community(db, id).await;
    }
    for (id, collection, key, value, field) in cases {
        let value = if key == "community_id" {
            community_id(value)
        } else if key == "market_id" {
            market_id(id)
        } else {
            value.to_string()
        };
        db.collection::<mongodb::bson::Document>(collection)
            .update_one(doc! {key: value}, doc! {"$unset": {field: ""}})
            .await
            .unwrap();
        assert!(
            db.validate_facility_markets(&[pair(&format!("facility-{id}"), &market_id(id))])
                .await
                .is_err(),
            "malformed {collection} record was treated as ineligible"
        );
    }
    stop_app(app).await;
}

#[tokio::test]
async fn unavailable_database_is_an_error_not_a_rejection() {
    let client = Client::with_uri_str(
        "mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=100&connectTimeoutMS=100",
    )
    .await
    .unwrap();
    let db = DatabaseWrapper(client.database("facility_validation"));
    assert!(db
        .validate_facility_markets(&[pair("facility-a", &market_id("a"))])
        .await
        .is_err());
}

#[tokio::test]
async fn http_and_ewds_return_the_same_bulk_validation_results() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    seed_community(db, "a").await;
    seed_community(db, "b").await;
    db.markets()
        .update_one(
            doc! {"market_id": &market_id("a")},
            doc! {"$set": {"market_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"}},
        )
        .await
        .unwrap();
    let mappings_before = db.ids().count_documents(doc! {}).await.unwrap();
    let requests = vec![
        pair("facility-a", "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
        pair("facility-a", "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"),
        pair("facility-a", &market_id("b")),
        pair("missing", &market_id("b")),
        pair("facility-b", "missing"),
        pair("facility-b", &market_id("b")),
        pair("facility-b", &market_id("b")),
    ];
    let expected = db.validate_facility_markets(&requests).await.unwrap();
    assert!(expected[0].eligible && expected[1].eligible);
    assert!(!expected[2].eligible && !expected[3].eligible && !expected[4].eligible);
    for batch in [requests, vec![]] {
        let expected = if batch.is_empty() {
            json!([])
        } else {
            json!(expected)
        };
        let http = reqwest::Client::new()
            .post(format!("{}/facilities/validate", app.address))
            .json(&batch)
            .send()
            .await
            .unwrap();
        assert_eq!(http.status(), reqwest::StatusCode::OK);
        assert_eq!(http.json::<Value>().await.unwrap(), expected);
        let ewds = validate_via_ewds(db, json!(batch)).await;
        assert_eq!(ewds["success"], true);
        assert_eq!(ewds["data"], expected);
    }
    assert_eq!(db.ids().count_documents(doc! {}).await.unwrap(), mappings_before);
    assert_eq!(db.orders().count_documents(doc! {}).await.unwrap(), 0);
    stop_app(app).await;
}

#[tokio::test]
async fn both_transports_reject_malformed_validation_requests() {
    let app = init_app().await;
    for payload in [
        json!({}),
        json!([{"facility_id": "a"}]),
        json!([{"facility_id": 1, "market_id": "a"}]),
    ] {
        let http = reqwest::Client::new()
            .post(format!("{}/facilities/validate", app.address))
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(http.status(), reqwest::StatusCode::BAD_REQUEST);
        let ewds = validate_via_ewds(&app.db_wrapper, payload).await;
        assert_eq!(ewds["success"], false);
        assert_eq!(ewds["data"], json!([]));
        assert_eq!(ewds["error"]["code"], "invalid_request");
    }
    stop_app(app).await;
}

#[tokio::test]
async fn both_transports_report_storage_errors_without_eligibility_results() {
    let app = init_app().await;
    seed_community(&app.db_wrapper, "a").await;
    app.db_wrapper
        .facilities()
        .update_one(
            doc! {"facility_id": "facility-a"},
            doc! {"$unset": {"site_id": ""}},
        )
        .await
        .unwrap();
    let payload = json!([pair("facility-a", &market_id("a"))]);
    let http = reqwest::Client::new()
        .post(format!("{}/facilities/validate", app.address))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(http.status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
    let http_error = http.json::<Value>().await.unwrap();
    let ewds = validate_via_ewds(&app.db_wrapper, payload).await;
    assert_eq!(ewds["success"], false);
    assert_eq!(ewds["data"], json!([]));
    assert_eq!(ewds["error"], http_error);
    assert_eq!(http_error["code"], "VALIDATION_UNAVAILABLE");
    stop_app(app).await;
}
