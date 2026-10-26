use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::db::DatabaseWrapper;
use mongodb::{bson::doc, Client};
use primitives::db_api_schema::grid_topology::{
    EnergyCommunitySchema, FacilityMarketValidationReason, FacilityMarketValidationRequest,
    FacilitySchema, SiteSchema,
};
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};

fn pair(facility_id: &str, market_id: &str) -> FacilityMarketValidationRequest {
    FacilityMarketValidationRequest {
        facility_id: facility_id.to_string(),
        market_id: market_id.to_string(),
    }
}

async fn seed_community(db: &DatabaseWrapper, id: &str) {
    db.markets()
        .insert_one(MarketSchema {
            market_id: format!("market-{id}"),
            community_id: id.to_string(),
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
            community_id: id.to_string(),
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
        .update_one(doc! {"community_id": "a"}, doc! {"$set": {"sites": []}})
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
