use std::collections::HashMap;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order};
use launchpad_matching_service::api::controller::{MatchControllerBase, DbMarketData};
use launchpad_matching_service::api::types::{DbBidOfferMatch, OrdersToMatch};
use async_trait::async_trait;

struct MockMatchController;

#[async_trait]
impl MatchControllerBase for MockMatchController {
    async fn insert_bid_offer_matches_to_db(&self, _matches: Vec<DbBidOfferMatch>) {
        // Mocked, does nothing
    }
    async fn update_market_statistics(
        &self, _market_data_map: HashMap<(String, u64), DbMarketData>) {
        // Mocked, does nothing
    }

}

#[tokio::test]
async fn test_process_market_id_for_pay_as_bid() {
    let controller = MockMatchController {};
    let user_id = "user1".to_string();
    let market_id = "market1".to_string();
    let area_uuid = "area1".to_string();

    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: area_uuid.clone(),
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

    let orders = vec![
        DbOrderSchema {
            _id: "bid1".to_string(),
            status: Default::default(),
            order: Order::Bid(bid),
        },
        DbOrderSchema {
            _id: "offer1".to_string(),
            status: Default::default(),
            order: Order::Offer(offer),
        },
    ];

    let orders_to_match = OrdersToMatch {
        orders: orders.clone(),
        user_id: user_id.clone(),
    };
    let result = controller.process_market_id_for_pay_as_bid(orders_to_match).await;
    
    assert!(result.contains_key(&market_id));
    assert_eq!(result.get(&market_id).unwrap().len(), 1);
    
    let match_obj = &result.get(&market_id).unwrap()[0];
    assert_eq!(match_obj.selected_energy, 10.0);
    assert_eq!(match_obj.energy_rate, 15.0);
    assert_eq!(match_obj.market_id, market_id);
}

#[tokio::test]
async fn test_process_market_id_multiple_orders() {
    let controller = MockMatchController;
    let market_id = "market1".to_string();

    let bid1 = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 20.0,
        },
    };

    let bid2 = DbBid {
        buyer: "buyer2".to_string(),
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

    let offer1 = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 15.0,
            energy_rate: 10.0,
        },
    };

    let offer2 = DbOffer {
        seller: "seller2".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 12.0,
        },
    };

    let orders = vec![
        DbOrderSchema {
            _id: "bid1".to_string(),
            status: Default::default(),
            order: Order::Bid(bid1),
        },
        DbOrderSchema {
            _id: "bid2".to_string(),
            status: Default::default(),
            order: Order::Bid(bid2),
        },
        DbOrderSchema {
            _id: "offer1".to_string(),
            status: Default::default(),
            order: Order::Offer(offer1),
        },
        DbOrderSchema {
            _id: "offer2".to_string(),
            status: Default::default(),
            order: Order::Offer(offer2),
        },
    ];

    let orders_to_match = OrdersToMatch {
        orders: orders.clone(),
        user_id: "user1".to_string().clone(),
    };
    let result = controller.process_market_id_for_pay_as_bid(orders_to_match).await;
    
    assert!(result.contains_key(&market_id));
    let matches = result.get(&market_id).unwrap();
    assert_eq!(matches.len(), 3);
    
    // Check match 1
    assert_eq!(matches[0].bid.buyer, "buyer1");
    assert_eq!(matches[0].offer.seller, "seller1");
    assert_eq!(matches[0].selected_energy, 10.0);
    assert_eq!(matches[0].energy_rate, 20.0);
    
    // Check match 2
    assert_eq!(matches[1].bid.buyer, "buyer2");
    assert_eq!(matches[1].offer.seller, "seller1");
    assert_eq!(matches[1].selected_energy, 5.0);
    assert_eq!(matches[1].energy_rate, 15.0);

    // Check match 3
    assert_eq!(matches[2].bid.buyer, "buyer2");
    assert_eq!(matches[2].offer.seller, "seller2");
    assert_eq!(matches[2].selected_energy, 5.0);
    assert_eq!(matches[2].energy_rate, 15.0);
}

#[tokio::test]
async fn test_process_market_id_one_bid_multiple_offers() {
    let controller = MockMatchController;
    let market_id = "market1".to_string();

    // One bid with 20.0 energy
    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 20.0,
            energy_rate: 25.0,
        },
    };

    // Offer1 with 12.0 energy (cheapest)
    let offer1 = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 12.0,
            energy_rate: 10.0,
        },
    };

    // Offer2 with 15.0 energy (more expensive but still matches)
    let offer2 = DbOffer {
        seller: "seller2".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 15.0,
            energy_rate: 15.0,
        },
    };

    let orders = vec![
        DbOrderSchema {
            _id: "bid1".to_string(),
            status: Default::default(),
            order: Order::Bid(bid),
        },
        DbOrderSchema {
            _id: "offer1".to_string(),
            status: Default::default(),
            order: Order::Offer(offer1),
        },
        DbOrderSchema {
            _id: "offer2".to_string(),
            status: Default::default(),
            order: Order::Offer(offer2),
        },
    ];

    let orders_to_match = OrdersToMatch {
        orders: orders.clone(),
        user_id: "user1".to_string().clone(),
    };
    let result = controller.process_market_id_for_pay_as_bid(orders_to_match).await;
    
    assert!(result.contains_key(&market_id));
    let matches = result.get(&market_id).unwrap();
    assert_eq!(matches.len(), 2);
    
    // Match 1
    assert_eq!(matches[0].bid.buyer, "buyer1");
    assert_eq!(matches[0].offer.seller, "seller1");
    assert_eq!(matches[0].selected_energy, 12.0);
    assert_eq!(matches[0].energy_rate, 25.0);
    
    // Match 2
    assert_eq!(matches[1].bid.buyer, "buyer1");
    assert_eq!(matches[1].offer.seller, "seller2");
    assert_eq!(matches[1].selected_energy, 8.0);
    assert_eq!(matches[1].energy_rate, 25.0);
}

#[tokio::test]
async fn test_calculate_market_statistics() {
    let controller = MockMatchController;
    let market_id = "market_stats".to_string();
    let time_slot = 100u64;

    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot,
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
            time_slot,
            creation_time: 100,
            energy: 15.0,
            energy_rate: 10.0,
        },
    };

    let orders = vec![
        DbOrderSchema {
            _id: "bid1".to_string(),
            status: Default::default(),
            order: Order::Bid(bid.clone()),
        },
        DbOrderSchema {
            _id: "offer1".to_string(),
            status: Default::default(),
            order: Order::Offer(offer.clone()),
        },
    ];

    let matches = vec![
        DbBidOfferMatch {
            user_id: "user1".to_string(),
            market_id: market_id.clone(),
            time_slot,
            bid: bid.clone(),
            offer: offer.clone(),
            residual_bid: None,
            residual_offer: Some(offer.clone()),
            selected_energy: 10.0,
            energy_rate: 15.0,
        }
    ];

    let stats_map = controller.calculate_market_statistics(&orders, &matches, "user1".to_string()).await;

    assert_eq!(stats_map.len(), 1);
    let stats = stats_map.get(&(market_id.clone(), time_slot)).unwrap();

    assert_eq!(stats.market_id, market_id);
    assert_eq!(stats.time_slot, time_slot);
    assert_eq!(stats.submitted_bid_count, 1);
    assert_eq!(stats.submitted_offer_count, 1);
    assert_eq!(stats.total_matches, 1);
    assert_eq!(stats.total_matched_energy_kWh, 10.0);
    
    // Total energy submitted: 10 (bid) + 15 (offer) = 25
    // Matched energy: 10
    // Unmatched should be: (10 - 10) + (15 - 10) = 5
    assert_eq!(stats.total_unmatched_energy_kWh, 5.0);
}
