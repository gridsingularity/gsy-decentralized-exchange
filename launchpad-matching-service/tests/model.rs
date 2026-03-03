use launchpad_matching_service::api::model::MatchModel;
use launchpad_matching_service::api::types::DbBidOfferMatch;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};

async fn setup(collection_name: &str) -> Option<MatchModel> {
    let model = MatchModel::new().await.ok()?.with_collection(collection_name);
    model.db.collection::<DbBidOfferMatch>(collection_name).drop(None).await.ok();
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

    let result = model.get_average_energy_rate_series(Some(market_id), 0, 1000).await;
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
            market_id: market_id.clone(),
            time_slot: 150,
            bid: DbBid { buyer: "b1".to_string(), nonce: 1, bid_component: DbOrderComponent { time_slot: 150, ..bid_comp.clone() } },
            offer: DbOffer { seller: "s1".to_string(), nonce: 1, offer_component: DbOrderComponent { time_slot: 150, ..offer_comp.clone() } },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
        // Match in range, target market
        DbBidOfferMatch {
            market_id: market_id.clone(),
            time_slot: 160,
            bid: DbBid { buyer: "b1".to_string(), nonce: 2, bid_component: DbOrderComponent { time_slot: 160, ..bid_comp.clone() } },
            offer: DbOffer { seller: "s1".to_string(), nonce: 2, offer_component: DbOrderComponent { time_slot: 160, ..offer_comp.clone() } },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
        // Match out of range (too early)
        DbBidOfferMatch {
            market_id: market_id.clone(),
            time_slot: 50,
            bid: DbBid { buyer: "b1".to_string(), nonce: 3, bid_component: DbOrderComponent { time_slot: 50, ..bid_comp.clone() } },
            offer: DbOffer { seller: "s1".to_string(), nonce: 3, offer_component: DbOrderComponent { time_slot: 50, ..offer_comp.clone() } },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
        // Match in range, different market
        DbBidOfferMatch {
            market_id: other_market.clone(),
            time_slot: 155,
            bid: DbBid { buyer: "b1".to_string(), nonce: 4, bid_component: DbOrderComponent { time_slot: 155, market_id: other_market.clone(), ..bid_comp.clone() } },
            offer: DbOffer { seller: "s1".to_string(), nonce: 4, offer_component: DbOrderComponent { time_slot: 155, market_id: other_market.clone(), ..offer_comp.clone() } },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
    ];

    model.insert_matches(matches).await.unwrap();

    // Test 1: Cumulative filtering (time range + market_id)
    let results = model.get_matches(100, 200, Some(market_id.clone()), 10).await.unwrap();
    assert_eq!(results.len(), 2, "Should find 2 matches for target market in time range");
    assert!(results.iter().all(|m| m.time_slot >= 100 && m.time_slot <= 200));
    assert!(results.iter().all(|m| m.market_id == market_id));

    // Test 2: Optional market_id (None should return both markets)
    let results_all_markets = model.get_matches(100, 200, None, 10).await.unwrap();
    assert_eq!(results_all_markets.len(), 3, "Should find 3 matches across all markets in time range");

    // Test 3: Limit
    let results_limited = model.get_matches(100, 200, None, 1).await.unwrap();
    assert_eq!(results_limited.len(), 1, "Should respect the limit of 1");
    // Since we sort by time_slot, it should be the one at 150
    assert_eq!(results_limited[0].time_slot, 150);
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

    model.insert_matches(vec![match_s100_1, match_s100_2, match_s200_1]).await.unwrap();

    let result = model.get_average_energy_rate_series(Some(market_id), 0, 1000).await;
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
    let result = model.get_average_energy_rate_series(None, 400, 600).await;
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
        market_id: market_id.clone(),
        time_slot: 300,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 35.0,
    };

    model.insert_matches(vec![match_s100, match_s200, match_s300]).await.unwrap();

    // Query range [150, 250] -> should only return slot 200
    let result = model.get_average_energy_rate_series(Some(market_id.clone()), 150, 250).await;
    let series = result.unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 200);

    // Query range [100, 200] -> should return slots 100 and 200
    let result = model.get_average_energy_rate_series(Some(market_id.clone()), 100, 200).await;
    let series = result.unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].time_slot, 100);
    assert_eq!(series[1].time_slot, 200);
}
