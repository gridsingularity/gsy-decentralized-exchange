use actix_web::{test, App};
use launchpad_matching_service::api::views;
use launchpad_matching_service::api::types::{OrdersToMatch, DbBidOfferMatch};
use launchpad_matching_service::api::controller::DbMarketData;
use launchpad_matching_service::api::model::{MatchModel, MarketStatisticsResponse};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, DbBid, DbOffer, DbOrderComponent};
use std::collections::HashMap;

async fn setup_db(collection_name: &str) -> Option<MatchModel> {
    let model = MatchModel::new().await.ok()?.with_collection(collection_name);
    model.db.collection::<DbBidOfferMatch>(collection_name).drop(None).await.ok();
    // Also drop market_data to have a clean state for statistics tests
    model.db.collection::<mongodb::bson::Document>("market_data").drop(None).await.ok();
    Some(model)
}

#[actix_web::test]
async fn test_health_check_endpoint() {
    let app = test::init_service(
        App::new().service(views::health_check)
    ).await;

    let req = test::TestRequest::get().uri("/health-check").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_pay_as_bid_endpoint() {
    // We use a specific collection name to avoid interference, 
    // though the controller currently uses a hardcoded one in some places, 
    // the MatchModel::new() will connect to the same DB.
    let _model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::pay_as_bid)
    ).await;

    let market_id = "view_test_market".to_string();
    let user_id = "view_test_user".to_string();

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

    let orders_to_match = OrdersToMatch {
        user_id: user_id.clone(),
        orders: vec![
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
        ],
    };

    let req = test::TestRequest::post()
        .uri("/match")
        .set_json(&orders_to_match)
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    assert!(body.contains_key(&market_id));
    assert_eq!(body.get(&market_id).unwrap().len(), 1);
    
    let m = &body.get(&market_id).unwrap()[0];
    assert_eq!(m.selected_energy, 10.0);
    assert_eq!(m.energy_rate, 15.0);
}

#[actix_web::test]
async fn test_filter_matches_endpoint() {
    let model = match setup_db("matches").await { // Use "matches" as it's the default collection
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::filter_matches)
    ).await;

    let market_id = "filter_test_market".to_string();
    let user_id = "filter_test_user".to_string();

    let bid_comp = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market_id.clone(),
        time_slot: 150,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 15.0,
    };
    let offer_comp = DbOrderComponent {
        area_uuid: "area2".to_string(),
        market_id: market_id.clone(),
        time_slot: 150,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 10.0,
    };

    let matches = vec![
        DbBidOfferMatch {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: 150,
            bid: DbBid { buyer: "b1".to_string(), nonce: 1, bid_component: bid_comp.clone() },
            offer: DbOffer { seller: "s1".to_string(), nonce: 1, offer_component: offer_comp.clone() },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
        DbBidOfferMatch {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: 160,
            bid: DbBid { buyer: "b1".to_string(), nonce: 2, bid_component: DbOrderComponent { time_slot: 160, ..bid_comp.clone() } },
            offer: DbOffer { seller: "s1".to_string(), nonce: 2, offer_component: DbOrderComponent { time_slot: 160, ..offer_comp.clone() } },
            residual_bid: None, residual_offer: None, selected_energy: 1.0, energy_rate: 10.0,
        },
    ];

    model.insert_matches(matches).await.unwrap();

    // Test filtering via query parameters
    let uri = format!(
        "/matches?user_id={}&market_id={}&start_time=100&end_time=200&limit=10",
        user_id, market_id
    );
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: Vec<DbBidOfferMatch> = test::read_body_json(resp).await;
    assert_eq!(body.len(), 2);

    // Test optional limit and market_id
    let uri_no_limit = format!(
        "/matches?user_id={}&start_time=100&end_time=200",
        user_id
    );
    let req = test::TestRequest::get().uri(&uri_no_limit).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body_no_limit: Vec<DbBidOfferMatch> = test::read_body_json(resp).await;
    assert_eq!(body_no_limit.len(), 2);
}

#[actix_web::test]
async fn test_get_market_statistics_endpoint() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::get_market_statistics)
    ).await;

    let market_id = "stats_test_market".to_string();
    let user_id = "stats_test_user".to_string();

    // 1. Insert some matches to have data for average trade rate
    let bid_comp = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market_id.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 20.0,
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
        DbBidOfferMatch {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: 100,
            bid: DbBid { buyer: "b1".to_string(), nonce: 1, bid_component: bid_comp.clone() },
            offer: DbOffer { seller: "s1".to_string(), nonce: 1, offer_component: offer_comp.clone() },
            residual_bid: None, residual_offer: None, selected_energy: 10.0, energy_rate: 20.0,
        },
    ];
    model.insert_matches(matches).await.unwrap();

    // 2. Insert some market data for energy timeseries
    let market_data = vec![
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
    ];
    model.upsert_market_data(market_data).await.unwrap();

    // 3. Test the endpoint
    let uri = format!(
        "/statistics?user_id={}&market_id={}&start_time=0&end_time=200",
        user_id, market_id
    );
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: MarketStatisticsResponse = test::read_body_json(resp).await;
    
    assert_eq!(body.total_matches, 1);
    assert!(body.success_rate > 0.0);
    assert_eq!(body.average_trade_rate_timeseries.len(), 1);
    assert_eq!(body.average_trade_rate_timeseries[0].average_energy_rate, 20.0);
    assert_eq!(body.energy_timeseries.len(), 1);
    assert_eq!(body.energy_timeseries[0].matched_energy_kWh, 10.0);
    assert_eq!(body.energy_timeseries[0].unmatched_energy_kWh, 5.0);
}

#[actix_web::test]
async fn test_get_market_statistics_optional_market_id() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::get_market_statistics)
    ).await;

    let market1 = "market1".to_string();
    let market2 = "market2".to_string();
    let user_id = "multi_market_user".to_string();

    // 1. Insert matches for two different markets
    let bid_comp1 = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market1.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 20.0,
    };
    let offer_comp1 = DbOrderComponent {
        area_uuid: "area2".to_string(),
        market_id: market1.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 10.0,
    };

    let bid_comp2 = DbOrderComponent {
        area_uuid: "area3".to_string(),
        market_id: market2.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 30.0,
    };
    let offer_comp2 = DbOrderComponent {
        area_uuid: "area4".to_string(),
        market_id: market2.clone(),
        time_slot: 100,
        creation_time: 100,
        energy: 10.0,
        energy_rate: 10.0,
    };

    let matches = vec![
        DbBidOfferMatch {
            user_id: user_id.clone(),
            market_id: market1.clone(),
            time_slot: 100,
            bid: DbBid { buyer: "b1".to_string(), nonce: 1, bid_component: bid_comp1.clone() },
            offer: DbOffer { seller: "s1".to_string(), nonce: 1, offer_component: offer_comp1.clone() },
            residual_bid: None, residual_offer: None, selected_energy: 10.0, energy_rate: 20.0,
        },
        DbBidOfferMatch {
            user_id: user_id.clone(),
            market_id: market2.clone(),
            time_slot: 100,
            bid: DbBid { buyer: "b2".to_string(), nonce: 1, bid_component: bid_comp2.clone() },
            offer: DbOffer { seller: "s2".to_string(), nonce: 1, offer_component: offer_comp2.clone() },
            residual_bid: None, residual_offer: None, selected_energy: 10.0, energy_rate: 30.0,
        },
    ];
    model.insert_matches(matches).await.unwrap();

    // 2. Insert market data for both markets
    let market_data = vec![
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market1.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market2.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
    ];
    model.upsert_market_data(market_data).await.unwrap();

    // 3. Test the endpoint WITHOUT market_id
    let uri = format!(
        "/statistics?user_id={}&start_time=0&end_time=200",
        user_id
    );
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: MarketStatisticsResponse = test::read_body_json(resp).await;
    
    // Total matches should be 1 + 1 = 2
    assert_eq!(body.total_matches, 2);
    
    // Average trade rate for time_slot 100 should be (20.0 + 30.0) / 2 = 25.0
    assert_eq!(body.average_trade_rate_timeseries.len(), 1);
    assert_eq!(body.average_trade_rate_timeseries[0].average_energy_rate, 25.0);
    
    // Energy timeseries for time_slot 100 should aggregate both markets
    // matched = 10.0 + 10.0 = 20.0
    // unmatched = 5.0 + 5.0 = 10.0
    assert_eq!(body.energy_timeseries.len(), 1);
    assert_eq!(body.energy_timeseries[0].matched_energy_kWh, 20.0);
    assert_eq!(body.energy_timeseries[0].unmatched_energy_kWh, 10.0);
    
    // Success rate should be 20.0 / (20.0 + 10.0) = 0.666...
    assert!((body.success_rate - 2.0/3.0).abs() < 0.0001);
}

#[actix_web::test]
async fn test_get_markets_endpoint() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::get_markets)
    ).await;

    let user_id = "market_id_test_user".to_string();
    let market1 = "market1".to_string();
    let market2 = "market2".to_string();

    // Insert market data for both markets
    let market_data = vec![
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market1.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market2.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
        // Same market, different time slot - should still only result in unique market_id
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market1.clone(),
            time_slot: 200,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
    ];
    model.upsert_market_data(market_data).await.unwrap();

    let uri = format!("/markets?user_id={}", user_id);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let mut body: Vec<String> = test::read_body_json(resp).await;
    body.sort();
    
    assert_eq!(body.len(), 2);
    assert_eq!(body[0], market1);
    assert_eq!(body[1], market2);
}

#[actix_web::test]
async fn test_get_markets_different_users() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new().service(views::get_markets)
    ).await;

    let user1 = "user1".to_string();
    let user2 = "user2".to_string();
    let market1 = "market1".to_string();
    let market2 = "market2".to_string();

    // Insert market data for both users
    let market_data = vec![
        DbMarketData {
            user_id: user1.clone(),
            market_id: market1.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
        DbMarketData {
            user_id: user2.clone(),
            market_id: market2.clone(),
            time_slot: 100,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
    ];
    model.upsert_market_data(market_data).await.unwrap();

    // Test for user1
    let uri1 = format!("/markets?user_id={}", user1);
    let req1 = test::TestRequest::get().uri(&uri1).to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert!(resp1.status().is_success());
    let body1: Vec<String> = test::read_body_json(resp1).await;
    assert_eq!(body1.len(), 1);
    assert_eq!(body1[0], market1);

    // Test for user2
    let uri2 = format!("/markets?user_id={}", user2);
    let req2 = test::TestRequest::get().uri(&uri2).to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert!(resp2.status().is_success());
    let body2: Vec<String> = test::read_body_json(resp2).await;
    assert_eq!(body2.len(), 1);
    assert_eq!(body2[0], market2);
}
