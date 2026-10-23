use async_trait::async_trait;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order,
};
use launchpad_matching_service::api::controller::{DbMarketData, MatchControllerBase};
use launchpad_matching_service::api::model;
use launchpad_matching_service::api::types::{DbBidOfferMatch, OrdersToMatch};
use mongodb::bson::oid::ObjectId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Mock controller: the database methods do nothing, the two write methods
/// count their calls.
#[derive(Default)]
struct MockMatchController {
    insert_calls: AtomicUsize,
    update_calls: AtomicUsize,
}

#[async_trait]
impl MatchControllerBase for MockMatchController {
    async fn insert_bid_offer_matches_to_db(
        &self,
        _matches: Vec<DbBidOfferMatch>,
    ) -> mongodb::error::Result<()> {
        self.insert_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn update_market_statistics_to_db(
        &self,
        _market_data_map: HashMap<(String, u64), DbMarketData>,
    ) -> mongodb::error::Result<()> {
        self.update_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn filter_matches(
        &self,
        _user_id: String,
        _market_id: Option<String>,
        _start_time: u64,
        _end_time: u64,
        _limit: Option<i64>,
        _offset: Option<u64>,
        _sort: model::SortOrder,
    ) -> mongodb::error::Result<Vec<DbBidOfferMatch>> {
        Ok(Vec::new())
    }

    async fn get_market_statistics(
        &self,
        _user_id: String,
        _market_id: Option<String>,
        _start_time: u64,
        _end_time: u64,
        _resolution: model::Resolution,
        _tz: String,
    ) -> mongodb::error::Result<model::MarketStatisticsResponse> {
        Ok(model::MarketStatisticsResponse {
            average_trade_rate_timeseries: Vec::new(),
            energy_timeseries: Vec::new(),
            total_matches: 0,
            success_rate: 0.0,
            active_traders: 0,
            markets: 0,
        })
    }

    async fn get_markets(&self, _user_id: String) -> mongodb::error::Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn test_process_market_id_for_pay_as_bid() {
    let controller = MockMatchController::default();
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
    let result = controller
        .process_market_id_for_pay_as_bid(orders_to_match, true)
        .await
        .expect("mocked DB calls never fail");

    assert!(result.contains_key(&market_id));
    assert_eq!(result.get(&market_id).unwrap().len(), 1);

    let match_obj = &result.get(&market_id).unwrap()[0];
    assert_eq!(match_obj.selected_energy, 10.0);
    assert_eq!(match_obj.energy_rate, 15.0);
    assert_eq!(match_obj.market_id, market_id);
}

#[tokio::test]
async fn test_process_market_id_multiple_orders() {
    let controller = MockMatchController::default();
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
    let result = controller
        .process_market_id_for_pay_as_bid(orders_to_match, true)
        .await
        .expect("mocked DB calls never fail");

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
    let controller = MockMatchController::default();
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
    let result = controller
        .process_market_id_for_pay_as_bid(orders_to_match, true)
        .await
        .expect("mocked DB calls never fail");

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
    let controller = MockMatchController::default();
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

    let matches = vec![DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: "user1".to_string(),
        market_id: market_id.clone(),
        time_slot,
        bid: bid.clone(),
        offer: offer.clone(),
        residual_bid: None,
        residual_offer: Some(offer.clone()),
        selected_energy: 10.0,
        energy_rate: 15.0,
    }];

    let stats_map = controller
        .calculate_market_statistics(&orders, &matches, "user1".to_string())
        .await;

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

fn bid_order(
    id: &str,
    buyer: &str,
    market_id: &str,
    time_slot: u64,
    energy: f64,
    energy_rate: f64,
) -> DbOrderSchema {
    DbOrderSchema {
        _id: id.to_string(),
        status: Default::default(),
        order: Order::Bid(DbBid {
            buyer: buyer.to_string(),
            nonce: 1,
            bid_component: DbOrderComponent {
                area_uuid: "area1".to_string(),
                market_id: market_id.to_string(),
                time_slot,
                creation_time: 100,
                energy,
                energy_rate,
            },
        }),
    }
}

fn offer_order(
    id: &str,
    seller: &str,
    market_id: &str,
    time_slot: u64,
    energy: f64,
    energy_rate: f64,
) -> DbOrderSchema {
    DbOrderSchema {
        _id: id.to_string(),
        status: Default::default(),
        order: Order::Offer(DbOffer {
            seller: seller.to_string(),
            nonce: 1,
            offer_component: DbOrderComponent {
                area_uuid: "area2".to_string(),
                market_id: market_id.to_string(),
                time_slot,
                creation_time: 100,
                energy,
                energy_rate,
            },
        }),
    }
}

/// MS-1 regression: one market with matches in two time slots of the same
/// batch must return the matches of both slots (the response used to keep
/// only the last iterated slot).
#[tokio::test]
async fn test_process_market_id_two_time_slots_returns_all_matches() {
    let controller = MockMatchController::default();
    let market_id = "market1";

    let orders = vec![
        bid_order("bid-100", "buyer1", market_id, 100, 10.0, 15.0),
        offer_order("offer-100", "seller1", market_id, 100, 10.0, 10.0),
        bid_order("bid-200", "buyer2", market_id, 200, 4.0, 20.0),
        offer_order("offer-200a", "seller2", market_id, 200, 3.0, 12.0),
        offer_order("offer-200b", "seller3", market_id, 200, 3.0, 13.0),
    ];

    let result = controller
        .process_market_id_for_pay_as_bid(
            OrdersToMatch {
                user_id: "user1".to_string(),
                orders,
            },
            true,
        )
        .await
        .expect("mocked DB calls never fail");

    assert_eq!(result.len(), 1);
    let matches = result.get(market_id).unwrap();
    assert_eq!(matches.len(), 3, "matches of both slots must be returned");

    // Slots come out in ascending order.
    assert_eq!(matches[0].time_slot, 100);
    assert_eq!(matches[0].selected_energy, 10.0);
    assert_eq!(matches[1].time_slot, 200);
    assert_eq!(matches[1].offer.seller, "seller2");
    assert_eq!(matches[1].selected_energy, 3.0);
    assert_eq!(matches[2].time_slot, 200);
    assert_eq!(matches[2].offer.seller, "seller3");
    assert_eq!(matches[2].selected_energy, 1.0);
}

/// Every market of the batch keeps a key in the response, even without
/// matches.
#[tokio::test]
async fn test_process_market_id_keeps_key_for_market_without_matches() {
    let controller = MockMatchController::default();

    let orders = vec![
        bid_order("bid1", "buyer1", "market1", 100, 10.0, 15.0),
        offer_order("offer1", "seller1", "market1", 100, 10.0, 10.0),
        // market2: offer more expensive than the bid, no match.
        bid_order("bid2", "buyer2", "market2", 100, 10.0, 5.0),
        offer_order("offer2", "seller2", "market2", 100, 10.0, 10.0),
    ];

    let result = controller
        .process_market_id_for_pay_as_bid(
            OrdersToMatch {
                user_id: "user1".to_string(),
                orders,
            },
            true,
        )
        .await
        .expect("mocked DB calls never fail");

    assert_eq!(result.len(), 2);
    assert_eq!(result.get("market1").unwrap().len(), 1);
    assert!(result.get("market2").unwrap().is_empty());
}

/// MS-6: every match carries a unique `match_id` and the `_id`s of the
/// submitted orders its bid and offer came from.
#[tokio::test]
async fn test_matches_carry_order_ids_and_unique_match_ids() {
    let controller = MockMatchController::default();
    let market_id = "market1";

    // Same setup as test_process_market_id_multiple_orders.
    let orders = vec![
        bid_order("bid-high", "buyer1", market_id, 100, 10.0, 20.0),
        bid_order("bid-low", "buyer2", market_id, 100, 10.0, 15.0),
        offer_order("offer-cheap", "seller1", market_id, 100, 15.0, 10.0),
        offer_order("offer-dear", "seller2", market_id, 100, 10.0, 12.0),
    ];

    let result = controller
        .process_market_id_for_pay_as_bid(
            OrdersToMatch {
                user_id: "user1".to_string(),
                orders,
            },
            true,
        )
        .await
        .expect("mocked DB calls never fail");

    let matches = result.get(market_id).unwrap();
    assert_eq!(matches.len(), 3);

    let ids: Vec<(&str, &str)> = matches
        .iter()
        .map(|m| (m.bid_order_id.as_str(), m.offer_order_id.as_str()))
        .collect();
    assert_eq!(
        ids,
        vec![
            ("bid-high", "offer-cheap"),
            ("bid-low", "offer-cheap"),
            ("bid-low", "offer-dear"),
        ]
    );

    let mut match_ids: Vec<&str> = matches.iter().map(|m| m.match_id.as_str()).collect();
    for match_id in &match_ids {
        assert_eq!(match_id.len(), 24);
        assert!(ObjectId::parse_str(match_id).is_ok());
    }
    match_ids.sort();
    match_ids.dedup();
    assert_eq!(match_ids.len(), 3, "match ids must be unique");
}

/// A dry run returns the same matches as a normal run but calls neither
/// write method; a normal run calls each once.
#[tokio::test]
async fn test_process_market_id_dry_run_does_not_persist() {
    let market_id = "market1";
    let orders = || {
        vec![
            bid_order("bid-high", "buyer1", market_id, 100, 10.0, 20.0),
            bid_order("bid-low", "buyer2", market_id, 100, 10.0, 15.0),
            offer_order("offer-cheap", "seller1", market_id, 100, 15.0, 10.0),
            offer_order("offer-dear", "seller2", market_id, 200, 10.0, 12.0),
            bid_order("bid-later", "buyer3", market_id, 200, 5.0, 14.0),
        ]
    };

    let dry = MockMatchController::default();
    let dry_result = dry
        .process_market_id_for_pay_as_bid(
            OrdersToMatch {
                user_id: "user1".to_string(),
                orders: orders(),
            },
            false,
        )
        .await
        .expect("mocked DB calls never fail");
    assert_eq!(dry.insert_calls.load(Ordering::SeqCst), 0);
    assert_eq!(dry.update_calls.load(Ordering::SeqCst), 0);

    let normal = MockMatchController::default();
    let normal_result = normal
        .process_market_id_for_pay_as_bid(
            OrdersToMatch {
                user_id: "user1".to_string(),
                orders: orders(),
            },
            true,
        )
        .await
        .expect("mocked DB calls never fail");
    assert_eq!(normal.insert_calls.load(Ordering::SeqCst), 1);
    assert_eq!(normal.update_calls.load(Ordering::SeqCst), 1);

    // Same matches, apart from the freshly generated match ids.
    let strip_ids = |result: &HashMap<String, Vec<DbBidOfferMatch>>| {
        let mut result = result.clone();
        for matches in result.values_mut() {
            for m in matches.iter_mut() {
                assert!(ObjectId::parse_str(&m.match_id).is_ok());
                m.match_id.clear();
            }
        }
        result
    };
    assert_eq!(dry_result.get(market_id).unwrap().len(), 3);
    assert_eq!(strip_ids(&dry_result), strip_ids(&normal_result));
}
