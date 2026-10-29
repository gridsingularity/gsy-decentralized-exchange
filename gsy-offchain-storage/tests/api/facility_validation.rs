use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::db::DatabaseWrapper;
use mongodb::{bson::doc, Client};
use primitives::db_api_schema::grid_topology::{
    EnergyCommunitySchema, FacilityMarketValidationReason, FacilityMarketValidationRequest,
    FacilitySchema, SiteSchema,
};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::ewds::dto::{EwdsRequestEnvelope, EwdsSendMessageDto};
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
    uuid::Uuid::from_bytes(primitives::utils::create_encrypted_bytes16_from_string(id)).to_string()
}

async fn seed_community(db: &DatabaseWrapper, id: &str) {
    db.markets()
        .insert_one(MarketSchema {
            market_id: format!("market-{id}"),
            community_id: community_id(id),
            opening_time: "100".to_string(),
            closing_time: "200".to_string(),
            delivery_start_time: "300".to_string(),
            delivery_end_time: "400".to_string(),
            market_type: MarketType::Spot,
            matching_algorithm: MatchingAlgorithm::PayAsBid,
            created_at: "100".to_string(),
        })
        .await
        .unwrap();
    db.communities()
        .upsert(EnergyCommunitySchema {
            community_id: community_id(id),
            community_name: id.to_string(),
            sites: vec![format!("site-{id}")],
        })
        .await
        .unwrap();
    db.sites()
        .upsert(SiteSchema {
            site_name: format!("site-{id}"),
            site_description: String::new(),
            facilities: vec![format!("facility-{id}")],
        })
        .await
        .unwrap();
    db.facilities()
        .upsert(FacilitySchema {
            facility_id: format!("facility-{id}"),
            facility_name: format!("facility-{id}"),
            site_id: format!("site-{id}"),
            owner_id: format!("owner-{id}"),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn http_community_ids_are_canonical_and_remain_eligible() {
    let app = init_app().await;
    seed_community(&app.db_wrapper, "a").await;
    let canonical = community_id("a");
    for id in [
        canonical.to_uppercase(),
        format!("0x{}", canonical.replace('-', "").to_uppercase()),
    ] {
        let response = reqwest::Client::new()
            .post(format!("{}/communities", app.address))
            .json(&EnergyCommunitySchema {
                community_id: id,
                community_name: "a".into(),
                sites: vec!["site-a".into()],
            })
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response
                .json::<EnergyCommunitySchema>()
                .await
                .unwrap()
                .community_id,
            canonical
        );
        let result =
            validate_via_ewds(&app.db_wrapper, json!([pair("facility-a", "market-a")])).await;
        assert_eq!(result["data"][0]["eligible"], true);
    }
    assert_eq!(
        app.db_wrapper
            .communities()
            .count_documents(doc! {})
            .await
            .unwrap(),
        1
    );
    let response = reqwest::Client::new()
        .post(format!("{}/communities", app.address))
        .json(&json!({"community_id": "not-a-uuid", "community_name": "invalid", "sites": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    stop_app(app).await;
}

#[tokio::test]
async fn http_topology_replacements_update_eligibility_and_preserve_name_conflicts() {
    let app = init_app().await;
    seed_community(&app.db_wrapper, "a").await;
    let client = reqwest::Client::new();
    let mut facility = FacilitySchema {
        facility_id: "facility-new".into(),
        facility_name: "New facility".into(),
        site_id: "site-a".into(),
        owner_id: "owner-new".into(),
    };
    for owner in ["owner-new", "owner-updated"] {
        facility.owner_id = owner.into();
        let response = client
            .post(format!("{}/facilities", app.address))
            .json(&facility)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.json::<FacilitySchema>().await.unwrap().owner_id,
            owner
        );
    }
    let requests = [
        pair("facility-new", "market-a"),
        pair("facility-a", "market-a"),
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
            .json(&SiteSchema {
                site_name: "site-a".into(),
                site_description: "Updated".into(),
                facilities,
            })
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
        .json(&facility)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        app.db_wrapper
            .facilities()
            .count_documents(doc! {})
            .await
            .unwrap(),
        2
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
    let requests = vec![
        pair("facility-a", "market-a"),
        pair("facility-a", "market-b"),
        pair("missing", "market-a"),
        pair("facility-b", "missing"),
        pair("facility-b", "market-b"),
        pair("owner-a", "market-a"),
        pair("facility-a", "market-a"),
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
    ]) {
        assert_eq!(result.facility_id, request.facility_id);
        assert_eq!(result.market_id, request.market_id);
        assert_eq!(result.eligible, reason.is_none());
        assert_eq!(result.reason, reason);
    }
    assert!(db.validate_facility_markets(&[]).await.unwrap().is_empty());
    assert_eq!(db.ids().count_documents(doc! {}).await.unwrap(), 1);
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
            "site_name",
            "site-no-site",
            doc! {},
            UnknownSite,
        ),
        (
            "no-facility-link",
            "sites",
            "site_name",
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
            .validate_facility_markets(&[pair(&format!("facility-{id}"), &format!("market-{id}"))])
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
    let requests = [pair("facility-a", "market-a")];
    assert!(!db.validate_facility_markets(&requests).await.unwrap()[0].eligible);
    seed_community(db, "a").await;
    assert!(db.validate_facility_markets(&requests).await.unwrap()[0].eligible);
    db.communities()
        .update_one(doc! {"community_id": community_id("a")}, doc! {"$set": {"sites": []}})
        .await
        .unwrap();
    assert!(!db.validate_facility_markets(&requests).await.unwrap()[0].eligible);
    stop_app(app).await;
}

#[tokio::test]
async fn malformed_database_records_are_errors_not_rejections() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    for (id, collection, key, value, field) in [
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
        ("site", "sites", "site_name", "site-site", "facilities"),
        (
            "community",
            "communities",
            "community_id",
            "community",
            "sites",
        ),
    ] {
        seed_community(db, id).await;
        let value = if key == "community_id" {
            community_id(value)
        } else {
            value.to_string()
        };
        db.collection::<mongodb::bson::Document>(collection)
            .update_one(doc! {key: value}, doc! {"$unset": {field: ""}})
            .await
            .unwrap();
        assert!(
            db.validate_facility_markets(&[pair(
                &format!("facility-{id}"),
                &format!("market-{id}")
            )])
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
        .validate_facility_markets(&[pair("facility-a", "market-a")])
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
            doc! {"market_id": "market-a"},
            doc! {"$set": {"market_id": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},
        )
        .await
        .unwrap();
    let requests = vec![
        pair("facility-a", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"),
        pair("facility-a", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        pair("facility-a", "market-b"),
        pair("missing", "market-b"),
        pair("facility-b", "missing"),
        pair("facility-b", "market-b"),
        pair("facility-b", "market-b"),
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
    assert_eq!(db.ids().count_documents(doc! {}).await.unwrap(), 0);
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
    let payload = json!([pair("facility-a", "market-a")]);
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
