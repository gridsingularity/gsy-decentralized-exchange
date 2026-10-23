use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use launchpad_matching_service::api::controller::DbMarketData;
use launchpad_matching_service::api::model::{
    EnergyTimeSeriesPoint, MatchModel, Resolution, SortOrder, TimeSeriesPoint, ensure_indexes,
};
use launchpad_matching_service::api::types::DbBidOfferMatch;
use launchpad_matching_service::configuration::get_configuration;
use mongodb::Client;
use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;

async fn setup(collection_name: &str) -> Option<MatchModel> {
    let model = MatchModel::new()
        .await
        .ok()?
        .with_collection(collection_name);
    model
        .db
        .collection::<DbBidOfferMatch>(collection_name)
        .drop(None)
        .await
        .ok();
    Some(model)
}

#[tokio::test]
async fn test_insert_matches() {
    let model = match setup("test_insert_matches").await {
        Some(m) => m,
        None => return,
    };

    let market_id = "test_market_insert".to_string();
    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 15.0,
        },
    };

    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let matches = vec![DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        bid,
        offer,
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 15.0,
    }];

    let result = model.insert_matches(matches).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_average_energy_rate_series() {
    let model = match setup("test_get_average_energy_rate_series").await {
        Some(m) => m,
        None => return,
    };

    let market_id = format!("test_market_series_{}", 12345);

    // Insert some test data
    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 200,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 15.0,
        },
    };
    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 200,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let match1 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 200,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 5.0,
        energy_rate: 20.0,
    };
    let match2 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 200,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 5.0,
        energy_rate: 10.0,
    };

    model.insert_matches(vec![match1, match2]).await.unwrap();

    let result = model
        .get_average_energy_rate_series(
            "user1".to_string(),
            Some(market_id),
            0,
            1000,
            Resolution::NoAggregation,
            "UTC",
        )
        .await;
    assert!(result.is_ok());
    let series = result.unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 200);
    assert_eq!(series[0].average_energy_rate, 15.0);
}

#[tokio::test]
async fn test_get_matches() {
    let model = match setup("test_get_matches").await {
        Some(m) => m,
        None => return,
    };

    let market_id = format!("test_get_matches_{}", 999);
    let other_market = format!("test_get_matches_other_{}", 888);

    // Common bid/offer components
    let bid_comp = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 15.0,
    };
    let offer_comp = DbOrderComponent {
        area_uuid: "area2".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 10.0,
    };

    let matches = vec![
        // Match in range, target market
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: "user1".to_string(),
            market_id: market_id.clone(),
            time_slot: 150,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 1,
                bid_component: DbOrderComponent {
                    time_slot: 150,
                    ..bid_comp.clone()
                },
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 1,
                offer_component: DbOrderComponent {
                    time_slot: 150,
                    ..offer_comp.clone()
                },
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 1.0,
            energy_rate: 10.0,
        },
        // Match in range, target market
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: "user1".to_string(),
            market_id: market_id.clone(),
            time_slot: 160,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 2,
                bid_component: DbOrderComponent {
                    time_slot: 160,
                    ..bid_comp.clone()
                },
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 2,
                offer_component: DbOrderComponent {
                    time_slot: 160,
                    ..offer_comp.clone()
                },
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 1.0,
            energy_rate: 10.0,
        },
        // Match out of range (too early)
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: "user1".to_string(),
            market_id: market_id.clone(),
            time_slot: 50,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 3,
                bid_component: DbOrderComponent {
                    time_slot: 50,
                    ..bid_comp.clone()
                },
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 3,
                offer_component: DbOrderComponent {
                    time_slot: 50,
                    ..offer_comp.clone()
                },
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 1.0,
            energy_rate: 10.0,
        },
        // Match in range, different market
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: "user1".to_string(),
            market_id: other_market.clone(),
            time_slot: 155,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 4,
                bid_component: DbOrderComponent {
                    time_slot: 155,
                    market_id: other_market.clone(),
                    ..bid_comp.clone()
                },
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 4,
                offer_component: DbOrderComponent {
                    time_slot: 155,
                    market_id: other_market.clone(),
                    ..offer_comp.clone()
                },
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 1.0,
            energy_rate: 10.0,
        },
    ];

    model.insert_matches(matches).await.unwrap();

    // Test 1: Cumulative filtering (time range + market_id)
    let results = model
        .get_matches(
            100,
            200,
            "user1".to_string(),
            Some(market_id.clone()),
            Some(10),
            None,
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert_eq!(
        results.len(),
        2,
        "Should find 2 matches for target market in time range"
    );
    assert!(
        results
            .iter()
            .all(|m| m.time_slot >= 100 && m.time_slot <= 200)
    );
    assert!(results.iter().all(|m| m.market_id == market_id));

    // Test 2: Optional market_id (None should return both markets)
    let results_all_markets = model
        .get_matches(
            100,
            200,
            "user1".to_string(),
            None,
            Some(10),
            None,
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert_eq!(
        results_all_markets.len(),
        3,
        "Should find 3 matches across all markets in time range"
    );

    // Test 3: Limit
    let results_limited = model
        .get_matches(
            100,
            200,
            "user1".to_string(),
            None,
            Some(1),
            None,
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert_eq!(results_limited.len(), 1, "Should respect the limit of 1");
    // Since we sort by time_slot, it should be the one at 150
    assert_eq!(results_limited[0].time_slot, 150);

    // Test 4: No limit
    let results_no_limit = model
        .get_matches(
            100,
            200,
            "user1".to_string(),
            None,
            None,
            None,
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert_eq!(
        results_no_limit.len(),
        3,
        "Should return all matches when limit is None"
    );
}

#[tokio::test]
async fn test_get_average_energy_rate_series_multiple_slots() {
    let model = match setup("test_get_average_energy_rate_series_multiple_slots").await {
        Some(m) => m,
        None => return,
    };

    let market_id = format!("test_market_multi_slots_{}", 67890);

    // Test data:
    // Time slot 100: 2 matches, average (10.0 + 20.0) / 2 = 15.0
    // Time slot 200: 1 match, average 25.0

    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 15.0,
        },
    };
    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let match_s100_1 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 5.0,
        energy_rate: 10.0,
    };
    let match_s100_2 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 5.0,
        energy_rate: 20.0,
    };

    let match_s200_1 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 200,
        bid: DbBid {
            bid_component: DbOrderComponent {
                time_slot: 200,
                ..bid.bid_component.clone()
            },
            ..bid.clone()
        },
        offer: DbOffer {
            offer_component: DbOrderComponent {
                time_slot: 200,
                ..offer.offer_component.clone()
            },
            ..offer.clone()
        },
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 25.0,
    };

    model
        .insert_matches(vec![match_s100_1, match_s100_2, match_s200_1])
        .await
        .unwrap();

    let result = model
        .get_average_energy_rate_series(
            "user1".to_string(),
            Some(market_id),
            0,
            1000,
            Resolution::NoAggregation,
            "UTC",
        )
        .await;
    assert!(result.is_ok());
    let series = result.unwrap();

    // Should have 2 time slots, sorted by time_slot ascending
    assert_eq!(series.len(), 2);

    assert_eq!(series[0].time_slot, 100);
    assert_eq!(series[0].average_energy_rate, 15.0);

    assert_eq!(series[1].time_slot, 200);
    assert_eq!(series[1].average_energy_rate, 25.0);
}

#[tokio::test]
async fn test_get_average_energy_rate_series_all_markets() {
    let model = match setup("test_get_average_energy_rate_series_all_markets").await {
        Some(m) => m,
        None => return,
    };

    let market_id1 = format!("test_market_all_1_{}", 111);
    let market_id2 = format!("test_market_all_2_{}", 222);

    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id1.clone(),
            time_slot: 500,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };
    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id1.clone(),
            time_slot: 500,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let match1 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id1,
        time_slot: 500,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 10.0,
    };

    let match2 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id2,
        time_slot: 500,
        bid: DbBid {
            bid_component: DbOrderComponent {
                market_id: "test_market_all_2_111".to_string(), // irrelevant for filter
                ..bid.bid_component.clone()
            },
            ..bid.clone()
        },
        offer: DbOffer {
            offer_component: DbOrderComponent {
                market_id: "test_market_all_2_111".to_string(), // irrelevant for filter
                ..offer.offer_component.clone()
            },
            ..offer.clone()
        },
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 20.0,
    };

    model.insert_matches(vec![match1, match2]).await.unwrap();

    // Query with None for market_id -> should average both markets
    let result = model
        .get_average_energy_rate_series(
            "user1".to_string(),
            None,
            400,
            600,
            Resolution::NoAggregation,
            "UTC",
        )
        .await;
    assert!(result.is_ok());
    let series = result.unwrap();

    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 500);
    // Average of 10.0 and 20.0 is 15.0
    assert_eq!(series[0].average_energy_rate, 15.0);
}

#[tokio::test]
async fn test_get_average_energy_rate_series_time_range() {
    let model = match setup("test_get_average_energy_rate_series_time_range").await {
        Some(m) => m,
        None => return,
    };

    let market_id = format!("test_market_range_{}", 54321);

    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 15.0,
        },
    };
    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let match_s100 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 15.0,
    };
    let match_s200 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 200,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 25.0,
    };
    let match_s300 = DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot: 300,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 35.0,
    };

    model
        .insert_matches(vec![match_s100, match_s200, match_s300])
        .await
        .unwrap();

    // Query range [150, 250] -> should only return slot 200
    let result = model
        .get_average_energy_rate_series(
            "user1".to_string(),
            Some(market_id.clone()),
            150,
            250,
            Resolution::NoAggregation,
            "UTC",
        )
        .await;
    let series = result.unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 200);

    // Query range [100, 200] -> should return slots 100 and 200
    let result = model
        .get_average_energy_rate_series(
            "user1".to_string(),
            Some(market_id.clone()),
            100,
            200,
            Resolution::NoAggregation,
            "UTC",
        )
        .await;
    let series = result.unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].time_slot, 100);
    assert_eq!(series[1].time_slot, 200);
}

#[tokio::test]
async fn test_upsert_market_data() {
    // We use setup but manually handle the market_data collection
    let model = match setup("dummy_collection").await {
        Some(m) => m,
        None => return,
    };

    // Clear market_data collection for this test
    model
        .db
        .collection::<DbMarketData>("market_data")
        .drop(None)
        .await
        .unwrap();

    let market_id = "test_market_upsert".to_string();
    let time_slot = 100u64;

    let initial_data = vec![DbMarketData {
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot,
        submitted_bid_count: 5,
        submitted_offer_count: 3,
        total_matches: 2,
        total_matched_energy_kWh: 20.0,
        total_unmatched_energy_kWh: 10.0,
    }];

    // First upsert (insert)
    model.upsert_market_data(initial_data).await.unwrap();

    let collection = model.db.collection::<DbMarketData>("market_data");
    let stored = collection
        .find_one(
            doc! { "market_id": &market_id, "time_slot": time_slot as i64 },
            None,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(stored.submitted_bid_count, 5);
    assert_eq!(stored.total_matched_energy_kWh, 20.0);

    // Second upsert (update/increment)
    let increment_data = vec![DbMarketData {
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot,
        submitted_bid_count: 2,
        submitted_offer_count: 1,
        total_matches: 1,
        total_matched_energy_kWh: 5.0,
        total_unmatched_energy_kWh: 2.0,
    }];

    model.upsert_market_data(increment_data).await.unwrap();

    let stored_updated = collection
        .find_one(
            doc! { "market_id": &market_id, "time_slot": time_slot as i64 },
            None,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(stored_updated.submitted_bid_count, 7); // 5 + 2
    assert_eq!(stored_updated.submitted_offer_count, 4); // 3 + 1
    assert_eq!(stored_updated.total_matches, 3); // 2 + 1
    assert_eq!(stored_updated.total_matched_energy_kWh, 25.0); // 20.0 + 5.0
    assert_eq!(stored_updated.total_unmatched_energy_kWh, 12.0); // 10.0 + 2.0
}

// 2025-01-01T00:00:00Z
const JAN_1_2025: u64 = 1_735_689_600;
// 2025-07-01T00:00:00Z
const JUL_1_2025: u64 = 1_751_328_000;
const HOUR: u64 = 3600;
const DAY: u64 = 86_400;

/// A model on its own database `<DATABASE_NAME>_<suffix>`, dropped first, so
/// tests that need `market_data` (a fixed collection name) do not race with
/// other tests running in parallel.
async fn isolated_model(suffix: &str) -> MatchModel {
    let config = get_configuration().expect("Failed to load configuration");
    let client = Client::with_uri_str(config.get_connection_string())
        .await
        .expect("Failed to create MongoDB client");
    let db = client.database(&format!("{}_{}", config.database_name, suffix));
    db.drop(None)
        .await
        .expect("Failed to drop the test database");
    MatchModel::from_database(db)
}

fn trade(
    market_id: &str,
    time_slot: u64,
    buyer: &str,
    seller: &str,
    energy_rate: f64,
) -> DbBidOfferMatch {
    trade_for_user("user1", market_id, time_slot, buyer, seller, energy_rate)
}

fn trade_for_user(
    user_id: &str,
    market_id: &str,
    time_slot: u64,
    buyer: &str,
    seller: &str,
    energy_rate: f64,
) -> DbBidOfferMatch {
    let component = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market_id.to_string(),
        time_slot,
        creation_time: 100,
        energy: 1.0,
        energy_rate,
    };
    DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        user_id: user_id.to_string(),
        market_id: market_id.to_string(),
        time_slot,
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        bid: DbBid {
            buyer: buyer.to_string(),
            nonce: 1,
            bid_component: component.clone(),
        },
        offer: DbOffer {
            seller: seller.to_string(),
            nonce: 1,
            offer_component: DbOrderComponent {
                area_uuid: "area2".to_string(),
                ..component
            },
        },
        residual_bid: None,
        residual_offer: None,
        selected_energy: 1.0,
        energy_rate,
    }
}

fn slot_data(
    user_id: &str,
    market_id: &str,
    time_slot: u64,
    total_matches: u64,
    matched: f64,
    unmatched: f64,
) -> DbMarketData {
    DbMarketData {
        user_id: user_id.to_string(),
        market_id: market_id.to_string(),
        time_slot,
        submitted_bid_count: 1,
        submitted_offer_count: 1,
        total_matches,
        total_matched_energy_kWh: matched,
        total_unmatched_energy_kWh: unmatched,
    }
}

async fn rate_series(model: &MatchModel, resolution: Resolution, tz: &str) -> Vec<TimeSeriesPoint> {
    model
        .get_average_energy_rate_series(
            "user1".to_string(),
            None,
            0,
            u32::MAX as u64,
            resolution,
            tz,
        )
        .await
        .unwrap()
}

async fn energy_series(
    model: &MatchModel,
    resolution: Resolution,
    tz: &str,
) -> Vec<EnergyTimeSeriesPoint> {
    model
        .get_energy_series(
            "user1".to_string(),
            None,
            0,
            u32::MAX as u64,
            resolution,
            tz,
        )
        .await
        .unwrap()
}

fn slots_of(series: &[TimeSeriesPoint]) -> Vec<u64> {
    series.iter().map(|p| p.time_slot).collect()
}

fn energy_slots_of(series: &[EnergyTimeSeriesPoint]) -> Vec<u64> {
    series.iter().map(|p| p.time_slot).collect()
}

#[tokio::test]
async fn test_statistics_quarter_hour_buckets_align_to_quarters() {
    let model = isolated_model("stats_qh").await;
    let base = JAN_1_2025 + 9 * HOUR; // 09:00 UTC

    // 09:00 and 09:05 -> 09:00; 09:20 -> 09:15; 09:44 -> 09:30; 09:46 -> 09:45
    let offsets = [0, 300, 1200, 2640, 2760];
    let rates = [10.0, 20.0, 30.0, 40.0, 50.0];
    let trades = offsets
        .iter()
        .zip(rates)
        .map(|(offset, rate)| trade("m1", base + offset, "b1", "s1", rate))
        .collect();
    model.insert_matches(trades).await.unwrap();
    model
        .upsert_market_data(
            offsets
                .iter()
                .map(|offset| slot_data("user1", "m1", base + offset, 1, 1.0, 0.5))
                .collect(),
        )
        .await
        .unwrap();

    let expected_buckets = vec![base, base + 900, base + 1800, base + 2700];

    let rates = rate_series(&model, Resolution::QuarterHour, "UTC").await;
    assert_eq!(slots_of(&rates), expected_buckets);
    assert!(rates.iter().all(|p| p.time_slot % 900 == 0));
    let averages: Vec<f64> = rates.iter().map(|p| p.average_energy_rate).collect();
    assert_eq!(averages, vec![15.0, 30.0, 40.0, 50.0]);

    let energy = energy_series(&model, Resolution::QuarterHour, "UTC").await;
    assert_eq!(energy_slots_of(&energy), expected_buckets);
    assert_eq!(energy[0].matched_energy_kWh, 2.0);
    assert_eq!(energy[0].unmatched_energy_kWh, 1.0);
    assert_eq!(energy[1].matched_energy_kWh, 1.0);

    // Whole-hour offsets keep the same quarter-hour boundaries.
    let zurich = rate_series(&model, Resolution::QuarterHour, "Europe/Zurich").await;
    assert_eq!(slots_of(&zurich), expected_buckets);
}

#[tokio::test]
async fn test_statistics_day_buckets_start_at_local_midnight() {
    let model = isolated_model("stats_day_tz").await;

    // Winter (CET, UTC+1): 12:00Z and 23:30Z on 2025-01-01.
    let jan_noon = JAN_1_2025 + 12 * HOUR;
    let jan_late = JAN_1_2025 + 23 * HOUR + 1800;
    // Summer (CEST, UTC+2): 22:30Z on 2025-07-01.
    let jul_late = JUL_1_2025 + 22 * HOUR + 1800;

    model
        .insert_matches(vec![
            trade("m1", jan_noon, "b1", "s1", 10.0),
            trade("m1", jan_late, "b1", "s1", 20.0),
            trade("m1", jul_late, "b1", "s1", 30.0),
        ])
        .await
        .unwrap();
    model
        .upsert_market_data(vec![
            slot_data("user1", "m1", jan_noon, 1, 1.0, 0.0),
            slot_data("user1", "m1", jan_late, 1, 2.0, 0.0),
            slot_data("user1", "m1", jul_late, 1, 4.0, 0.0),
        ])
        .await
        .unwrap();

    // UTC: both January slots fall on 2025-01-01.
    let utc = rate_series(&model, Resolution::Day, "UTC").await;
    assert_eq!(slots_of(&utc), vec![JAN_1_2025, JUL_1_2025]);
    assert_eq!(utc[0].average_energy_rate, 15.0);
    let utc_energy = energy_series(&model, Resolution::Day, "UTC").await;
    assert_eq!(energy_slots_of(&utc_energy), vec![JAN_1_2025, JUL_1_2025]);
    assert_eq!(utc_energy[0].matched_energy_kWh, 3.0);

    // Europe/Zurich: local midnight is 23:00Z in winter and 22:00Z in summer,
    // so 23:30Z on 2025-01-01 already belongs to 2025-01-02.
    let expected = vec![
        JAN_1_2025 - HOUR,      // 2024-12-31T23:00Z
        JAN_1_2025 + 23 * HOUR, // 2025-01-01T23:00Z
        JUL_1_2025 + 22 * HOUR, // 2025-07-01T22:00Z
    ];
    let zurich = rate_series(&model, Resolution::Day, "Europe/Zurich").await;
    assert_eq!(slots_of(&zurich), expected);
    let averages: Vec<f64> = zurich.iter().map(|p| p.average_energy_rate).collect();
    assert_eq!(averages, vec![10.0, 20.0, 30.0]);
    let zurich_energy = energy_series(&model, Resolution::Day, "Europe/Zurich").await;
    assert_eq!(energy_slots_of(&zurich_energy), expected);
    let matched: Vec<f64> = zurich_energy.iter().map(|p| p.matched_energy_kWh).collect();
    assert_eq!(matched, vec![1.0, 2.0, 4.0]);
}

#[tokio::test]
async fn test_statistics_month_buckets_are_calendar_months() {
    let model = isolated_model("stats_month").await;

    let jan_31_noon = JAN_1_2025 + 30 * DAY + 12 * HOUR;
    let jan_31_late = JAN_1_2025 + 30 * DAY + 23 * HOUR + 1800;
    let feb_15 = JAN_1_2025 + 45 * DAY;
    let feb_1 = JAN_1_2025 + 31 * DAY;

    model
        .insert_matches(vec![
            trade("m1", jan_31_noon, "b1", "s1", 10.0),
            trade("m1", jan_31_late, "b1", "s1", 20.0),
            trade("m1", feb_15, "b1", "s1", 40.0),
        ])
        .await
        .unwrap();

    // UTC: 2025-01-01 and 2025-02-01 (not 30-day epoch buckets).
    let utc = rate_series(&model, Resolution::Month, "UTC").await;
    assert_eq!(slots_of(&utc), vec![JAN_1_2025, feb_1]);
    assert_eq!(utc[0].average_energy_rate, 15.0);
    assert_eq!(utc[1].average_energy_rate, 40.0);

    // Europe/Zurich: 23:30Z on Jan 31 is already February locally.
    let zurich = rate_series(&model, Resolution::Month, "Europe/Zurich").await;
    assert_eq!(slots_of(&zurich), vec![JAN_1_2025 - HOUR, feb_1 - HOUR]);
    assert_eq!(zurich[0].average_energy_rate, 10.0);
    assert_eq!(zurich[1].average_energy_rate, 30.0);
}

/// The bucket average is the mean over all trades in the bucket, not the
/// mean of per-slot means: 3 trades at 10 and 1 trade at 30 give 15, not 20.
#[tokio::test]
async fn test_statistics_average_rate_is_mean_over_trades() {
    let model = isolated_model("stats_avg").await;
    let slot_a = JAN_1_2025 + HOUR;
    let slot_b = JAN_1_2025 + 2 * HOUR;

    model
        .insert_matches(vec![
            trade("m1", slot_a, "b1", "s1", 10.0),
            trade("m1", slot_a, "b2", "s1", 10.0),
            trade("m1", slot_a, "b3", "s1", 10.0),
            trade("m1", slot_b, "b1", "s2", 30.0),
        ])
        .await
        .unwrap();

    let per_slot = rate_series(&model, Resolution::NoAggregation, "UTC").await;
    assert_eq!(slots_of(&per_slot), vec![slot_a, slot_b]);
    assert_eq!(per_slot[0].average_energy_rate, 10.0);
    assert_eq!(per_slot[1].average_energy_rate, 30.0);

    let per_day = rate_series(&model, Resolution::Day, "UTC").await;
    assert_eq!(slots_of(&per_day), vec![JAN_1_2025]);
    assert_eq!(per_day[0].average_energy_rate, 15.0);
}

#[tokio::test]
async fn test_statistics_active_traders_and_markets() {
    let model = isolated_model("stats_counts").await;
    let slot = JAN_1_2025 + HOUR;
    let end = slot + HOUR;

    model
        .insert_matches(vec![
            trade("m1", slot, "b1", "s1", 10.0),
            trade("m1", slot, "b2", "s1", 10.0),
            // b2 also sells: counted once.
            trade("m2", slot, "b1", "b2", 10.0),
            // Outside the window.
            trade("m1", end + 1, "late_buyer", "late_seller", 10.0),
            // Other user.
            trade_for_user("user2", "m1", slot, "other_buyer", "other_seller", 10.0),
        ])
        .await
        .unwrap();
    model
        .upsert_market_data(vec![
            slot_data("user1", "m1", slot, 2, 2.0, 1.0),
            slot_data("user1", "m2", slot, 1, 1.0, 1.0),
            slot_data("user1", "m3", end + 1, 1, 1.0, 1.0),
            slot_data("user2", "m4", slot, 1, 1.0, 1.0),
        ])
        .await
        .unwrap();

    let traders = model
        .count_active_traders("user1".to_string(), None, 0, end)
        .await
        .unwrap();
    assert_eq!(traders, 3, "b1, b2 and s1");
    let totals = model
        .get_market_data_totals("user1".to_string(), None, 0, end)
        .await
        .unwrap();
    assert_eq!(totals.markets, 2);
    assert_eq!(totals.total_matches, 3);
    assert_eq!(totals.total_matched_energy_kWh, 3.0);
    assert_eq!(totals.total_unmatched_energy_kWh, 2.0);

    // Market filter.
    let traders_m2 = model
        .count_active_traders("user1".to_string(), Some("m2".to_string()), 0, end)
        .await
        .unwrap();
    assert_eq!(traders_m2, 2, "b1 and b2");
    let totals_m2 = model
        .get_market_data_totals("user1".to_string(), Some("m2".to_string()), 0, end)
        .await
        .unwrap();
    assert_eq!(totals_m2.markets, 1);

    // Empty window.
    let traders_none = model
        .count_active_traders("nobody".to_string(), None, 0, end)
        .await
        .unwrap();
    assert_eq!(traders_none, 0);
    let totals_none = model
        .get_market_data_totals("nobody".to_string(), None, 0, end)
        .await
        .unwrap();
    assert_eq!(totals_none.markets, 0);
    assert_eq!(totals_none.total_matches, 0);
}

#[tokio::test]
async fn test_get_matches_offset_pages_and_sort() {
    let model = isolated_model("matches_paging").await;

    let slots = [100, 100, 100, 200, 200, 300, 400];
    let trades: Vec<DbBidOfferMatch> = slots
        .iter()
        .map(|slot| trade("m1", *slot, "b1", "s1", 10.0))
        .collect();
    model.insert_matches(trades).await.unwrap();

    let all_asc = model
        .get_matches(
            0,
            1000,
            "user1".to_string(),
            None,
            None,
            None,
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert_eq!(all_asc.len(), slots.len());
    let keys: Vec<(u64, String)> = all_asc
        .iter()
        .map(|m| (m.time_slot, m.match_id.clone()))
        .collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys, "ascending by time_slot, then match_id");

    let mut paged = Vec::new();
    for offset in [0, 3, 6] {
        let page = model
            .get_matches(
                0,
                1000,
                "user1".to_string(),
                None,
                Some(3),
                Some(offset),
                SortOrder::Asc,
            )
            .await
            .unwrap();
        paged.extend(page.into_iter().map(|m| m.match_id));
    }
    let all_ids: Vec<String> = all_asc.iter().map(|m| m.match_id.clone()).collect();
    assert_eq!(paged, all_ids, "pages do not overlap and cover the set");

    let all_desc = model
        .get_matches(
            0,
            1000,
            "user1".to_string(),
            None,
            None,
            None,
            SortOrder::Desc,
        )
        .await
        .unwrap();
    let desc_ids: Vec<String> = all_desc.iter().map(|m| m.match_id.clone()).collect();
    let mut reversed = all_ids.clone();
    reversed.reverse();
    assert_eq!(desc_ids, reversed);

    let past_end = model
        .get_matches(
            0,
            1000,
            "user1".to_string(),
            None,
            Some(3),
            Some(7),
            SortOrder::Asc,
        )
        .await
        .unwrap();
    assert!(past_end.is_empty());
}

#[tokio::test]
async fn test_ensure_indexes_creates_expected_indexes() {
    let model = isolated_model("indexes").await;

    // An existing database with the index of earlier versions.
    model
        .db
        .collection::<mongodb::bson::Document>("matches")
        .create_index(
            mongodb::IndexModel::builder()
                .keys(doc! { "user_id": 1, "time_slot": 1 })
                .build(),
            None,
        )
        .await
        .unwrap();

    // Idempotent: a second run (old index already gone) succeeds too.
    ensure_indexes(&model.db).await.unwrap();
    ensure_indexes(&model.db).await.unwrap();

    async fn unique_by_name(
        db: &mongodb::Database,
        collection: &str,
    ) -> std::collections::HashMap<String, bool> {
        use futures_util::StreamExt;
        let mut cursor = db
            .collection::<mongodb::bson::Document>(collection)
            .list_indexes(None)
            .await
            .unwrap();
        let mut indexes = std::collections::HashMap::new();
        while let Some(index) = cursor.next().await {
            let index = index.unwrap();
            let options = index.options.unwrap_or_default();
            indexes.insert(options.name.unwrap(), options.unique.unwrap_or(false));
        }
        indexes
    }

    let matches = unique_by_name(&model.db, "matches").await;
    assert_eq!(
        matches.get("user_id_1_time_slot_1_match_id_1"),
        Some(&false)
    );
    assert_eq!(
        matches.get("user_id_1_time_slot_1"),
        None,
        "the obsolete index is dropped"
    );
    assert_eq!(
        matches.get("user_id_1_market_id_1_time_slot_1"),
        Some(&false)
    );
    assert_eq!(matches.get("match_id_1"), Some(&true));

    assert_eq!(
        matches.len(),
        4,
        "_id plus the three indexes: {:?}",
        matches
    );

    let market_data = unique_by_name(&model.db, "market_data").await;
    assert_eq!(
        market_data.get("user_id_1_market_id_1_time_slot_1"),
        Some(&true)
    );
    assert_eq!(market_data.get("user_id_1_time_slot_1"), Some(&false));
}

/// On a fresh database there is no obsolete index to drop; that is not an
/// error.
#[tokio::test]
async fn test_ensure_indexes_on_fresh_database() {
    let model = isolated_model("indexes_fresh").await;
    ensure_indexes(&model.db).await.unwrap();
    let names = model
        .db
        .collection::<mongodb::bson::Document>("matches")
        .list_index_names()
        .await
        .unwrap();
    assert!(names.contains(&"user_id_1_time_slot_1_match_id_1".to_string()));
    assert!(!names.contains(&"user_id_1_time_slot_1".to_string()));
}
