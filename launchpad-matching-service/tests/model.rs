use launchpad_matching_service::api::model::MatchModel;
use launchpad_matching_service::api::types::DbBidOfferMatch;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};

#[tokio::test]
async fn test_insert_matches() {
    let model = match MatchModel::new().await {
        Ok(m) => m,
        Err(_) => return, // Skip if no MongoDB
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
    let model = match MatchModel::new().await {
        Ok(m) => m,
        Err(_) => return, // Skip if no MongoDB
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

    let result = model.get_average_energy_rate_series(market_id, 0, 1000).await;
    assert!(result.is_ok());
    let series = result.unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 200);
    assert_eq!(series[0].average_energy_rate, 15.0);
}

#[tokio::test]
async fn test_get_average_energy_rate_series_multiple_slots() {
    let model = match MatchModel::new().await {
        Ok(m) => m,
        Err(_) => return, // Skip if no MongoDB
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

    let result = model.get_average_energy_rate_series(market_id, 0, 1000).await;
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
async fn test_get_average_energy_rate_series_time_range() {
    let model = match MatchModel::new().await {
        Ok(m) => m,
        Err(_) => return, // Skip if no MongoDB
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
    let result = model.get_average_energy_rate_series(market_id.clone(), 150, 250).await;
    let series = result.unwrap();
    assert_eq!(series.len(), 1);
    assert_eq!(series[0].time_slot, 200);

    // Query range [100, 200] -> should return slots 100 and 200
    let result = model.get_average_energy_rate_series(market_id.clone(), 100, 200).await;
    let series = result.unwrap();
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].time_slot, 100);
    assert_eq!(series[1].time_slot, 200);
}
