use actix_web::{test, App};
use launchpad_matching_service::api::views;
use launchpad_matching_service::api::types::{OrdersToMatch, DbBidOfferMatch};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, DbBid, DbOffer, DbOrderComponent};
use launchpad_matching_service::api::model::MatchModel;
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
    let _model = match setup_db("matches_test_views").await {
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
