use actix_web::http::StatusCode;
use actix_web::{App, test, web};
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order,
};
use launchpad_matching_service::api::controller::DbMarketData;
use launchpad_matching_service::api::model::{MarketStatisticsResponse, MatchModel};
use launchpad_matching_service::api::types::{DbBidOfferMatch, OrdersToMatch};
use launchpad_matching_service::api::views;
use launchpad_matching_service::auth::jwt::{JwtSecret, encode_jwt};
use launchpad_matching_service::configuration::{DEFAULT_JSON_BODY_LIMIT, get_configuration};
use mongodb::bson::oid::ObjectId;
use std::collections::HashMap;

const TEST_JWT_SECRET: &str = "test_secret_for_unit_tests";

fn test_bearer_token() -> String {
    let token = encode_jwt("test_user", TEST_JWT_SECRET, 1).expect("Failed to encode JWT");
    format!("Bearer {}", token)
}

async fn setup_db(collection_name: &str) -> Option<MatchModel> {
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
    // Also drop market_data to have a clean state for statistics tests
    model
        .db
        .collection::<mongodb::bson::Document>("market_data")
        .drop(None)
        .await
        .ok();
    Some(model)
}

fn create_dummy_bid(energy_rate: f64, energy: f64) -> DbBid {
    DbBid {
        buyer: "test_buyer".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: "test_market".to_string(),
            time_slot: 100,
            creation_time: 100,
            energy,
            energy_rate,
        },
    }
}

fn create_dummy_offer(energy_rate: f64, energy: f64) -> DbOffer {
    DbOffer {
        seller: "test_seller".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: "test_market".to_string(),
            time_slot: 100,
            creation_time: 100,
            energy,
            energy_rate,
        },
    }
}

#[actix_web::test]
async fn test_health_check_endpoint() {
    let app = test::init_service(App::new().service(views::health_check)).await;

    let req = test::TestRequest::get().uri("/health-check").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_ready_endpoint() {
    let model = match MatchModel::new().await {
        Ok(m) => m,
        Err(_) => return,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(model.db.clone()))
            .service(views::ready),
    )
    .await;

    let req = test::TestRequest::get().uri("/ready").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}

/// A database handle whose server cannot be reached; operations fail fast.
/// Creating it does not need MongoDB (the driver connects lazily).
async fn unreachable_database() -> mongodb::Database {
    let client = mongodb::Client::with_uri_str(
        "mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=200&connectTimeoutMS=200",
    )
    .await
    .expect("Failed to parse the MongoDB URI");
    client.database("launchpad_matching_service_unreachable")
}

#[actix_web::test]
async fn test_ready_endpoint_database_unavailable() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(unreachable_database().await))
            .service(views::ready),
    )
    .await;

    let req = test::TestRequest::get().uri("/ready").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["error"], "database_unavailable");
    assert_eq!(body["message"], "The database is not reachable.");
}

#[actix_web::test]
async fn test_endpoints_return_500_when_database_unavailable() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(unreachable_database().await))
            .service(views::pay_as_bid)
            .service(views::filter_matches)
            .service(views::get_market_statistics)
            .service(views::get_markets),
    )
    .await;

    let orders_to_match = OrdersToMatch {
        user_id: "unavailable_db_user".to_string(),
        orders: vec![
            DbOrderSchema {
                _id: "bid1".to_string(),
                status: Default::default(),
                order: Order::Bid(create_dummy_bid(15.0, 10.0)),
            },
            DbOrderSchema {
                _id: "offer1".to_string(),
                status: Default::default(),
                order: Order::Offer(create_dummy_offer(10.0, 10.0)),
            },
        ],
    };
    let mut requests = Vec::new();
    for uri in ["/match", "/match?dry_run=false"] {
        requests.push((
            uri,
            test::TestRequest::post()
                .uri(uri)
                .insert_header(("Authorization", test_bearer_token()))
                .set_json(&orders_to_match)
                .to_request(),
        ));
    }
    for uri in [
        "/matches?user_id=unavailable_db_user&start_time=0&end_time=200",
        "/statistics?user_id=unavailable_db_user&start_time=0&end_time=200",
        // A connectivity failure stays 500 even with a non-UTC time zone.
        "/statistics?user_id=unavailable_db_user&start_time=0&end_time=200&resolution=day&tz=Europe/Zurich",
        "/markets?user_id=unavailable_db_user",
    ] {
        requests.push((
            uri,
            test::TestRequest::get()
                .uri(uri)
                .insert_header(("Authorization", test_bearer_token()))
                .to_request(),
        ));
    }

    for (path, req) in requests {
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected status for {}",
            path
        );
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "internal_error", "for {}", path);
        assert_eq!(
            body["message"], "A database operation failed.",
            "for {}",
            path
        );
    }
}

#[actix_web::test]
async fn test_pay_as_bid_endpoint() {
    // We use a specific collection name to avoid interference,
    // though the controller currently uses a hardcoded one in some places,
    // the handler uses the same database through app_data.
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::pay_as_bid),
    )
    .await;

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
        .insert_header(("Authorization", test_bearer_token()))
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
    assert_eq!(m.bid_order_id, "bid1");
    assert_eq!(m.offer_order_id, "offer1");
    assert!(ObjectId::parse_str(&m.match_id).is_ok());
}

#[actix_web::test]
async fn test_filter_matches_endpoint() {
    let model = match setup_db("matches").await {
        // Use "matches" as it's the default collection
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::filter_matches),
    )
    .await;

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
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: 150,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 1,
                bid_component: bid_comp.clone(),
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 1,
                offer_component: offer_comp.clone(),
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 1.0,
            energy_rate: 10.0,
        },
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
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
    ];

    model.insert_matches(matches).await.unwrap();

    // Test filtering via query parameters
    let uri = format!(
        "/matches?user_id={}&market_id={}&start_time=100&end_time=200&limit=10",
        user_id, market_id
    );
    let req = test::TestRequest::get()
        .uri(&uri)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: Vec<DbBidOfferMatch> = test::read_body_json(resp).await;
    assert_eq!(body.len(), 2);

    // Test optional limit and market_id
    let uri_no_limit = format!("/matches?user_id={}&start_time=100&end_time=200", user_id);
    let req = test::TestRequest::get()
        .uri(&uri_no_limit)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
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
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::get_market_statistics),
    )
    .await;

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

    let matches = vec![DbBidOfferMatch {
        match_id: ObjectId::new().to_hex(),
        bid_order_id: "bid1".to_string(),
        offer_order_id: "offer1".to_string(),
        user_id: user_id.clone(),
        market_id: market_id.clone(),
        time_slot: 100,
        bid: DbBid {
            buyer: "b1".to_string(),
            nonce: 1,
            bid_component: bid_comp.clone(),
        },
        offer: DbOffer {
            seller: "s1".to_string(),
            nonce: 1,
            offer_component: offer_comp.clone(),
        },
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 20.0,
    }];
    model.insert_matches(matches).await.unwrap();

    // 2. Insert some market data for energy timeseries
    let market_data = vec![DbMarketData {
        user_id: user_id.clone(),
        market_id: market_id.clone(),
        time_slot: 100,
        submitted_bid_count: 1,
        submitted_offer_count: 1,
        total_matches: 1,
        total_matched_energy_kWh: 10.0,
        total_unmatched_energy_kWh: 5.0,
    }];
    model.upsert_market_data(market_data).await.unwrap();

    // 3. Test the endpoint
    let uri = format!(
        "/statistics?user_id={}&market_id={}&start_time=0&end_time=200",
        user_id, market_id
    );
    let req = test::TestRequest::get()
        .uri(&uri)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: MarketStatisticsResponse = test::read_body_json(resp).await;

    assert_eq!(body.total_matches, 1);
    assert!(body.success_rate > 0.0);
    assert_eq!(body.average_trade_rate_timeseries.len(), 1);
    assert_eq!(
        body.average_trade_rate_timeseries[0].average_energy_rate,
        20.0
    );
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
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::get_market_statistics),
    )
    .await;

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
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market1.clone(),
            time_slot: 100,
            bid: DbBid {
                buyer: "b1".to_string(),
                nonce: 1,
                bid_component: bid_comp1.clone(),
            },
            offer: DbOffer {
                seller: "s1".to_string(),
                nonce: 1,
                offer_component: offer_comp1.clone(),
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 10.0,
            energy_rate: 20.0,
        },
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market2.clone(),
            time_slot: 100,
            bid: DbBid {
                buyer: "b2".to_string(),
                nonce: 1,
                bid_component: bid_comp2.clone(),
            },
            offer: DbOffer {
                seller: "s2".to_string(),
                nonce: 1,
                offer_component: offer_comp2.clone(),
            },
            residual_bid: None,
            residual_offer: None,
            selected_energy: 10.0,
            energy_rate: 30.0,
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
    let uri = format!("/statistics?user_id={}&start_time=0&end_time=200", user_id);
    let req = test::TestRequest::get()
        .uri(&uri)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: MarketStatisticsResponse = test::read_body_json(resp).await;

    // Total matches should be 1 + 1 = 2
    assert_eq!(body.total_matches, 2);

    // Average trade rate for time_slot 100 should be (20.0 + 30.0) / 2 = 25.0
    assert_eq!(body.average_trade_rate_timeseries.len(), 1);
    assert_eq!(
        body.average_trade_rate_timeseries[0].average_energy_rate,
        25.0
    );

    // Energy timeseries for time_slot 100 should aggregate both markets
    // matched = 10.0 + 10.0 = 20.0
    // unmatched = 5.0 + 5.0 = 10.0
    assert_eq!(body.energy_timeseries.len(), 1);
    assert_eq!(body.energy_timeseries[0].matched_energy_kWh, 20.0);
    assert_eq!(body.energy_timeseries[0].unmatched_energy_kWh, 10.0);

    // Success rate should be 20.0 / (20.0 + 10.0) = 0.666...
    assert!((body.success_rate - 2.0 / 3.0).abs() < 0.0001);
}

#[actix_web::test]
async fn test_get_markets_endpoint() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::get_markets),
    )
    .await;

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
    let req = test::TestRequest::get()
        .uri(&uri)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
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
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::get_markets),
    )
    .await;

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
    let req1 = test::TestRequest::get()
        .uri(&uri1)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert!(resp1.status().is_success());
    let body1: Vec<String> = test::read_body_json(resp1).await;
    assert_eq!(body1.len(), 1);
    assert_eq!(body1[0], market1);

    // Test for user2
    let uri2 = format!("/markets?user_id={}", user2);
    let req2 = test::TestRequest::get()
        .uri(&uri2)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert!(resp2.status().is_success());
    let body2: Vec<String> = test::read_body_json(resp2).await;
    assert_eq!(body2.len(), 1);
    assert_eq!(body2[0], market2);
}

#[actix_web::test]
async fn test_get_market_statistics_resolution() {
    let model = match setup_db("matches").await {
        Some(m) => m,
        None => return,
    };
    // setup_db already drops "matches" and "market_data"

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(model.db.clone()))
            .service(views::get_market_statistics),
    )
    .await;

    let user_id = "res_test_user".to_string();
    let market_id = "res_test_market".to_string();

    // Create data across two different days
    let day1 = 1735689600; // 2025-01-01 00:00:00 UTC
    let day2 = 1735776000; // 2025-01-02 00:00:00 UTC

    // 1. Matches for trade rate
    let matches = vec![
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day1 + 3600, // 1 AM
            bid: create_dummy_bid(20.0, 10.0),
            offer: create_dummy_offer(15.0, 10.0),
            residual_bid: None,
            residual_offer: None,
            selected_energy: 10.0,
            energy_rate: 20.0,
        },
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day1 + 7200, // 2 AM
            bid: create_dummy_bid(30.0, 10.0),
            offer: create_dummy_offer(25.0, 10.0),
            residual_bid: None,
            residual_offer: None,
            selected_energy: 10.0,
            energy_rate: 30.0,
        },
        DbBidOfferMatch {
            match_id: ObjectId::new().to_hex(),
            bid_order_id: "bid1".to_string(),
            offer_order_id: "offer1".to_string(),
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day2 + 3600, // Day 2, 1 AM
            bid: create_dummy_bid(40.0, 10.0),
            offer: create_dummy_offer(35.0, 10.0),
            residual_bid: None,
            residual_offer: None,
            selected_energy: 10.0,
            energy_rate: 40.0,
        },
    ];
    model.insert_matches(matches).await.unwrap();

    // 2. Market data for energy statistics
    let market_data = vec![
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day1 + 3600,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 10.0,
            total_unmatched_energy_kWh: 5.0,
        },
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day1 + 7200,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 15.0,
            total_unmatched_energy_kWh: 5.0,
        },
        DbMarketData {
            user_id: user_id.clone(),
            market_id: market_id.clone(),
            time_slot: day2 + 3600,
            submitted_bid_count: 1,
            submitted_offer_count: 1,
            total_matches: 1,
            total_matched_energy_kWh: 20.0,
            total_unmatched_energy_kWh: 10.0,
        },
    ];
    model.upsert_market_data(market_data).await.unwrap();

    // Test Day resolution
    let uri = format!(
        "/statistics?user_id={}&market_id={}&start_time={}&end_time={}&resolution=day",
        user_id,
        market_id,
        day1,
        day2 + 86400
    );
    let req = test::TestRequest::get()
        .uri(&uri)
        .insert_header(("Authorization", test_bearer_token()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: MarketStatisticsResponse = test::read_body_json(resp).await;

    // Day 1 average trade rate: (20 + 30) / 2 = 25
    // Day 2 average trade rate: 40
    assert_eq!(body.average_trade_rate_timeseries.len(), 2);
    assert_eq!(body.average_trade_rate_timeseries[0].time_slot, day1);
    assert_eq!(
        body.average_trade_rate_timeseries[0].average_energy_rate,
        25.0
    );
    assert_eq!(body.average_trade_rate_timeseries[1].time_slot, day2);
    assert_eq!(
        body.average_trade_rate_timeseries[1].average_energy_rate,
        40.0
    );

    // Day 1 energy: matched = 10 + 15 = 25, unmatched = 5 + 5 = 10
    // Day 2 energy: matched = 20, unmatched = 10
    assert_eq!(body.energy_timeseries.len(), 2);
    assert_eq!(body.energy_timeseries[0].time_slot, day1);
    assert_eq!(body.energy_timeseries[0].matched_energy_kWh, 25.0);
    assert_eq!(body.energy_timeseries[0].unmatched_energy_kWh, 10.0);
    assert_eq!(body.energy_timeseries[1].time_slot, day2);
    assert_eq!(body.energy_timeseries[1].matched_energy_kWh, 20.0);
    assert_eq!(body.energy_timeseries[1].unmatched_energy_kWh, 10.0);
}

// 2025-01-01T00:00:00Z
const JAN_1_2025: u64 = 1_735_689_600;

/// A database `<DATABASE_NAME>_<suffix>`, dropped first, so these tests do not
/// race with the tests above that drop the shared `matches` and
/// `market_data` collections.
async fn isolated_db(suffix: &str) -> mongodb::Database {
    let config = get_configuration().expect("Failed to load configuration");
    let client = mongodb::Client::with_uri_str(config.get_connection_string())
        .await
        .expect("Failed to create MongoDB client");
    let db = client.database(&format!("{}_{}", config.database_name, suffix));
    db.drop(None)
        .await
        .expect("Failed to drop the test database");
    db
}

fn trade(
    user_id: &str,
    market_id: &str,
    time_slot: u64,
    buyer: &str,
    seller: &str,
) -> DbBidOfferMatch {
    let component = DbOrderComponent {
        area_uuid: "area1".to_string(),
        market_id: market_id.to_string(),
        time_slot,
        creation_time: 100,
        energy: 1.0,
        energy_rate: 10.0,
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
        energy_rate: 10.0,
    }
}

/// Authenticated GET; returns the status and the JSON body. A macro because
/// the service type returned by `test::init_service` cannot be named here.
macro_rules! get_json {
    ($app:expr, $uri:expr) => {{
        let req = test::TestRequest::get()
            .uri($uri)
            .insert_header(("Authorization", test_bearer_token()))
            .to_request();
        let resp = test::call_service($app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        (status, body)
    }};
}

#[actix_web::test]
async fn test_get_market_statistics_time_zone_and_counts() {
    let db = isolated_db("view_stats_tz").await;
    let model = MatchModel::from_database(db.clone());
    let user_id = "tz_user";

    // 2025-01-01T23:30Z is 2025-01-02 00:30 in Zurich (CET, UTC+1).
    let late = JAN_1_2025 + 23 * 3600 + 1800;
    model
        .insert_matches(vec![
            trade(user_id, "m1", late, "b1", "s1"),
            trade(user_id, "m2", late, "b2", "s1"),
        ])
        .await
        .unwrap();
    model
        .upsert_market_data(vec![
            DbMarketData {
                user_id: user_id.to_string(),
                market_id: "m1".to_string(),
                time_slot: late,
                submitted_bid_count: 1,
                submitted_offer_count: 1,
                total_matches: 1,
                total_matched_energy_kWh: 1.0,
                total_unmatched_energy_kWh: 1.0,
            },
            DbMarketData {
                user_id: user_id.to_string(),
                market_id: "m2".to_string(),
                time_slot: late,
                submitted_bid_count: 1,
                submitted_offer_count: 1,
                total_matches: 1,
                total_matched_energy_kWh: 1.0,
                total_unmatched_energy_kWh: 1.0,
            },
        ])
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(db.clone()))
            .app_data(views::query_config())
            .service(views::get_market_statistics),
    )
    .await;

    let window = format!(
        "/statistics?user_id={}&start_time={}&end_time={}",
        user_id,
        JAN_1_2025,
        JAN_1_2025 + 86_400
    );

    let (status, utc) = get_json!(&app, &format!("{}&resolution=day", window));
    assert_eq!(status, StatusCode::OK);
    let utc: MarketStatisticsResponse = serde_json::from_value(utc).unwrap();
    assert_eq!(utc.energy_timeseries.len(), 1);
    assert_eq!(utc.energy_timeseries[0].time_slot, JAN_1_2025);
    assert_eq!(utc.average_trade_rate_timeseries[0].time_slot, JAN_1_2025);
    assert_eq!(utc.active_traders, 3);
    assert_eq!(utc.markets, 2);
    assert_eq!(utc.total_matches, 2);
    assert_eq!(utc.success_rate, 0.5);

    let (status, zurich) = get_json!(&app, &format!("{}&resolution=day&tz=Europe/Zurich", window));
    assert_eq!(status, StatusCode::OK);
    let zurich: MarketStatisticsResponse = serde_json::from_value(zurich).unwrap();
    let local_midnight = JAN_1_2025 + 23 * 3600;
    assert_eq!(zurich.energy_timeseries[0].time_slot, local_midnight);
    assert_eq!(zurich.energy_timeseries[0].matched_energy_kWh, 2.0);
    assert_eq!(
        zurich.average_trade_rate_timeseries[0].time_slot,
        local_midnight
    );

    let (status, quarter) = get_json!(&app, &format!("{}&resolution=quarter_hour", window));
    assert_eq!(status, StatusCode::OK);
    let quarter: MarketStatisticsResponse = serde_json::from_value(quarter).unwrap();
    assert_eq!(
        quarter.energy_timeseries[0].time_slot,
        JAN_1_2025 + 23 * 3600 + 1800
    );

    // no_aggregation is unchanged: one point per time_slot.
    let (status, raw) = get_json!(&app, &window);
    assert_eq!(status, StatusCode::OK);
    let raw: MarketStatisticsResponse = serde_json::from_value(raw).unwrap();
    assert_eq!(raw.energy_timeseries.len(), 1);
    assert_eq!(raw.energy_timeseries[0].time_slot, late);
}

#[actix_web::test]
async fn test_get_market_statistics_invalid_time_zone() {
    let db = isolated_db("view_stats_bad_tz").await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(db))
            .app_data(views::query_config())
            .service(views::get_market_statistics),
    )
    .await;

    let window = "/statistics?user_id=bad_tz_user&start_time=0&end_time=200";
    let too_long = "A".repeat(65);
    for tz in [
        // Plausible shape, unknown to MongoDB.
        "Mars/Olympus",
        // Wrong case: MongoDB only knows "UTC".
        "utc",
        // Fails the shape check.
        "Europe/Zurich%3Bx",
        "%2B05%3A30",
        too_long.as_str(),
    ] {
        let (status, body) = get_json!(&app, &format!("{}&resolution=day&tz={}", window, tz));
        assert_eq!(status, StatusCode::BAD_REQUEST, "tz {}", tz);
        assert_eq!(body["error"], "invalid_timezone", "tz {}", tz);
        assert_eq!(body["message"], "Unknown time zone.", "tz {}", tz);
    }

    // An empty tz fails the shape check for every resolution.
    let (status, body) = get_json!(&app, &format!("{}&tz=", window));
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_timezone");
}

#[actix_web::test]
async fn test_filter_matches_offset_and_sort_endpoint() {
    let db = isolated_db("view_matches_paging").await;
    let model = MatchModel::from_database(db.clone());
    let user_id = "paging_user";
    let slots = [100, 100, 200, 300, 300, 300, 400];
    model
        .insert_matches(
            slots
                .iter()
                .map(|slot| trade(user_id, "m1", *slot, "b1", "s1"))
                .collect(),
        )
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(db))
            .app_data(views::query_config())
            .service(views::filter_matches),
    )
    .await;

    let base = format!("/matches?user_id={}&start_time=0&end_time=1000", user_id);
    let ids = |body: serde_json::Value| -> Vec<String> {
        let matches: Vec<DbBidOfferMatch> = serde_json::from_value(body).unwrap();
        matches.into_iter().map(|m| m.match_id).collect()
    };

    let (status, all) = get_json!(&app, &base);
    assert_eq!(status, StatusCode::OK);
    let all_asc = ids(all);
    assert_eq!(all_asc.len(), slots.len());

    let mut paged = Vec::new();
    for offset in [0, 2, 4, 6] {
        let (status, page) = get_json!(&app, &format!("{}&limit=2&offset={}", base, offset));
        assert_eq!(status, StatusCode::OK);
        paged.extend(ids(page));
    }
    assert_eq!(paged, all_asc, "pages do not overlap and cover the set");

    let (_, explicit_asc) = get_json!(&app, &format!("{}&sort=asc", base));
    assert_eq!(ids(explicit_asc), all_asc);

    let (status, desc) = get_json!(&app, &format!("{}&sort=desc", base));
    assert_eq!(status, StatusCode::OK);
    let desc_matches: Vec<DbBidOfferMatch> = serde_json::from_value(desc).unwrap();
    let desc_slots: Vec<u64> = desc_matches.iter().map(|m| m.time_slot).collect();
    assert_eq!(desc_slots, vec![400, 300, 300, 300, 200, 100, 100]);
    let mut reversed = all_asc.clone();
    reversed.reverse();
    let desc_ids: Vec<String> = desc_matches.into_iter().map(|m| m.match_id).collect();
    assert_eq!(desc_ids, reversed);

    let (status, desc_page) = get_json!(&app, &format!("{}&sort=desc&limit=3&offset=1", base));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(desc_page), reversed[1..4].to_vec());
}

#[actix_web::test]
async fn test_malformed_match_body_returns_json_400() {
    // Rejected before any database access.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(unreachable_database().await))
            .app_data(views::json_config(DEFAULT_JSON_BODY_LIMIT))
            .service(views::pay_as_bid),
    )
    .await;

    for body in [
        r#"{"user_id": "u1""#,
        r#"{"user_id": 5, "orders": []}"#,
        r#"{"orders": []}"#,
    ] {
        let req = test::TestRequest::post()
            .uri("/match")
            .insert_header(("Authorization", test_bearer_token()))
            .insert_header(("Content-Type", "application/json"))
            .set_payload(body)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "body {}", body);
        let json: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(json["error"], "invalid_request", "body {}", body);
        assert!(
            !json["message"].as_str().unwrap_or("").is_empty(),
            "body {}",
            body
        );
    }

    // The serde error text is passed through.
    let req = test::TestRequest::post()
        .uri("/match")
        .insert_header(("Authorization", test_bearer_token()))
        .insert_header(("Content-Type", "application/json"))
        .set_payload(r#"{"orders": []}"#)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let json: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        json["message"].as_str().unwrap().contains("user_id"),
        "message: {}",
        json["message"]
    );
}

#[actix_web::test]
async fn test_bad_query_strings_return_json_400() {
    // Rejected before any database access.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(unreachable_database().await))
            .app_data(views::query_config())
            .service(views::filter_matches)
            .service(views::get_market_statistics)
            .service(views::get_markets),
    )
    .await;

    for uri in [
        "/statistics?user_id=u1&start_time=abc&end_time=200",
        "/statistics?user_id=u1&start_time=0&end_time=200&resolution=week",
        "/statistics?start_time=0&end_time=200",
        "/matches?user_id=u1&start_time=0&end_time=200&sort=sideways",
        "/matches?user_id=u1&start_time=0&end_time=200&offset=-1",
        "/markets",
    ] {
        let (status, body) = get_json!(&app, uri);
        assert_eq!(status, StatusCode::BAD_REQUEST, "uri {}", uri);
        assert_eq!(body["error"], "invalid_request", "uri {}", uri);
        assert!(
            !body["message"].as_str().unwrap_or("").is_empty(),
            "uri {}",
            uri
        );
    }
}

/// Orders of a small batch for the dry-run tests: two matches in slot 100,
/// one in slot 200.
fn dry_run_orders(user_id: &str, market_id: &str) -> OrdersToMatch {
    let component = |area: &str, time_slot: u64, energy: f64, energy_rate: f64| DbOrderComponent {
        area_uuid: area.to_string(),
        market_id: market_id.to_string(),
        time_slot,
        creation_time: 100,
        energy,
        energy_rate,
    };
    let bid =
        |id: &str, buyer: &str, time_slot: u64, energy: f64, energy_rate: f64| DbOrderSchema {
            _id: id.to_string(),
            status: Default::default(),
            order: Order::Bid(DbBid {
                buyer: buyer.to_string(),
                nonce: 1,
                bid_component: component("area1", time_slot, energy, energy_rate),
            }),
        };
    let offer =
        |id: &str, seller: &str, time_slot: u64, energy: f64, energy_rate: f64| DbOrderSchema {
            _id: id.to_string(),
            status: Default::default(),
            order: Order::Offer(DbOffer {
                seller: seller.to_string(),
                nonce: 1,
                offer_component: component("area2", time_slot, energy, energy_rate),
            }),
        };
    OrdersToMatch {
        user_id: user_id.to_string(),
        orders: vec![
            bid("bid-high", "buyer1", 100, 10.0, 20.0),
            bid("bid-low", "buyer2", 100, 10.0, 15.0),
            offer("offer-cheap", "seller1", 100, 15.0, 10.0),
            bid("bid-later", "buyer3", 200, 5.0, 14.0),
            offer("offer-later", "seller2", 200, 10.0, 12.0),
        ],
    }
}

async fn count_documents(db: &mongodb::Database, collection: &str) -> u64 {
    db.collection::<mongodb::bson::Document>(collection)
        .count_documents(None, None)
        .await
        .expect("Failed to count documents")
}

#[actix_web::test]
async fn test_pay_as_bid_dry_run_writes_nothing() {
    let db = isolated_db("view_match_dry_run").await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(db.clone()))
            .app_data(views::query_config())
            .service(views::pay_as_bid),
    )
    .await;
    let market_id = "dry_run_market";
    let orders = dry_run_orders("dry_run_user", market_id);

    let post = |uri: &str| {
        test::TestRequest::post()
            .uri(uri)
            .insert_header(("Authorization", test_bearer_token()))
            .set_json(&orders)
            .to_request()
    };

    // Dry run: the matches come back, nothing is stored.
    let resp = test::call_service(&app, post("/match?dry_run=true")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let dry: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    let dry_matches = dry.get(market_id).expect("market key missing");
    assert_eq!(dry_matches.len(), 3);
    for m in dry_matches {
        assert!(ObjectId::parse_str(&m.match_id).is_ok());
    }
    assert_eq!(count_documents(&db, "matches").await, 0);
    assert_eq!(count_documents(&db, "market_data").await, 0);

    // The same batch without dry_run is stored.
    let resp = test::call_service(&app, post("/match")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stored: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    let stored_matches = stored.get(market_id).expect("market key missing");
    assert_eq!(count_documents(&db, "matches").await, 3);
    // One market_data document per (market_id, time_slot).
    assert_eq!(count_documents(&db, "market_data").await, 2);

    // Same matches apart from the match ids, which the dry run did not
    // persist.
    let key = |m: &DbBidOfferMatch| {
        (
            m.time_slot,
            m.bid_order_id.clone(),
            m.offer_order_id.clone(),
            m.selected_energy,
            m.energy_rate,
        )
    };
    let dry_keys: Vec<_> = dry_matches.iter().map(key).collect();
    let stored_keys: Vec<_> = stored_matches.iter().map(key).collect();
    assert_eq!(dry_keys, stored_keys);

    let model = MatchModel::from_database(db.clone());
    let persisted = model
        .get_matches(
            0,
            1_000,
            "dry_run_user".to_string(),
            None,
            None,
            None,
            Default::default(),
        )
        .await
        .expect("Failed to read the stored matches");
    let mut persisted_ids: Vec<String> = persisted.into_iter().map(|m| m.match_id).collect();
    let mut stored_ids: Vec<String> = stored_matches.iter().map(|m| m.match_id.clone()).collect();
    persisted_ids.sort();
    stored_ids.sort();
    assert_eq!(persisted_ids, stored_ids);
    for m in dry_matches {
        assert!(!persisted_ids.contains(&m.match_id));
    }
}

/// A dry run does not touch the database, so it succeeds even when MongoDB
/// is unreachable.
#[actix_web::test]
async fn test_pay_as_bid_dry_run_without_database() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(unreachable_database().await))
            .app_data(views::query_config())
            .service(views::pay_as_bid),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/match?dry_run=true")
        .insert_header(("Authorization", test_bearer_token()))
        .set_json(dry_run_orders("dry_run_user", "dry_run_market"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: HashMap<String, Vec<DbBidOfferMatch>> = test::read_body_json(resp).await;
    assert_eq!(body.get("dry_run_market").unwrap().len(), 3);
}

#[actix_web::test]
async fn test_pay_as_bid_invalid_dry_run_returns_json_400() {
    // Rejected before any database access.
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(unreachable_database().await))
            .app_data(views::query_config())
            .service(views::pay_as_bid),
    )
    .await;
    for uri in [
        "/match?dry_run=maybe",
        "/match?dry_run=1",
        "/match?dry_run=",
    ] {
        let req = test::TestRequest::post()
            .uri(uri)
            .insert_header(("Authorization", test_bearer_token()))
            .set_json(dry_run_orders("dry_run_user", "dry_run_market"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "uri {}", uri);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "invalid_request", "uri {}", uri);
        assert!(
            !body["message"].as_str().unwrap_or("").is_empty(),
            "uri {}",
            uri
        );
    }
}

fn command_error(code: i32, code_name: &str) -> mongodb::error::Error {
    let command_error: mongodb::error::CommandError = mongodb::bson::from_document(
        mongodb::bson::doc! { "code": code, "codeName": code_name, "errmsg": "test" },
    )
    .unwrap();
    mongodb::error::Error::from(mongodb::error::ErrorKind::Command(command_error))
}

// `test` is actix_web::test in this file, hence an async test.
#[actix_web::test]
async fn test_is_unknown_time_zone_error_classification() {
    assert!(views::is_unknown_time_zone_error(&command_error(
        40485,
        "Location40485"
    )));
    assert!(!views::is_unknown_time_zone_error(&command_error(
        241,
        "ConversionFailure"
    )));
    assert!(!views::is_unknown_time_zone_error(&command_error(
        2, "BadValue"
    )));
    let not_a_command_error =
        mongodb::error::Error::from(std::io::Error::other("connection reset"));
    assert!(!views::is_unknown_time_zone_error(&not_a_command_error));
}

/// A command error other than "unknown time zone" stays a 500 database error,
/// even with a non-UTC time zone. Trigger: a time_slot so large that
/// `$toDate` overflows (ConversionFailure).
#[actix_web::test]
async fn test_other_command_error_with_time_zone_is_500() {
    let db = isolated_db("view_stats_other_err").await;
    let model = MatchModel::from_database(db.clone());
    let huge_slot: u64 = 10_000_000_000_000_000;
    model
        .insert_matches(vec![trade("other_err_user", "m1", huge_slot, "b1", "s1")])
        .await
        .unwrap();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(db))
            .app_data(views::query_config())
            .service(views::get_market_statistics),
    )
    .await;

    let (status, body) = get_json!(
        &app,
        &format!(
            "/statistics?user_id=other_err_user&start_time=0&end_time={}&resolution=day&tz=Europe/Zurich",
            huge_slot * 2
        )
    );
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "internal_error");
    assert_eq!(body["message"], "A database operation failed.");
}
