use crate::helpers::init_app;
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order as DbOrder, OrderStatus,
};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid, Order, OrderComponent, OrderSchema,
};
use gsy_offchain_primitives::utils::h256_to_string;
use mongodb::bson::Bson;
use std::collections::HashMap;
use subxt::config::{substrate::BlakeTwo256, Hasher as HashT};
use subxt::utils::{AccountId32, H256};

pub fn create_test_accountid() -> AccountId32 {
    // A fixed 32-byte value, typically derived from a public key
    let account_id_bytes = [
        0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];

    AccountId32::from(account_id_bytes)
}

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    let app = init_app().await;
    let address = app.address;

    let account: AccountId32 = create_test_accountid();
    let order_id = H256::random();
    let market_id = H256::random();
    let area_id = H256::random();

    let bid = Bid {
        buyer: account,
        nonce: 1,
        bid_component: OrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: area_id,
            market_id: market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let bid_id = h256_to_string(BlakeTwo256.hash_of(&bid));

    let order = OrderSchema {
        _id: order_id,
        status: OrderStatus::Expired,
        order: Order::Bid(bid.clone()),
    };

    let orderlist = vec![order.clone()];
    let body = Vec::<OrderSchema<AccountId32, H256>>::encode(&orderlist);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("Failed to execute request.");

    let status = resp.status();
    let response = resp.json::<HashMap<usize, Bson>>().await.unwrap();

    let db = web::Data::new(app.db_wrapper);

    let resp_order_id = response.get(&0).unwrap();
    assert_eq!(resp_order_id.as_str().unwrap().to_string(), bid_id);
    let saved = db
        .get_ref()
        .orders()
        .get_order_by_id(resp_order_id)
        .await
        .unwrap();

    assert_eq!(200, status.as_u16());
    assert_eq!(saved.unwrap()._id, bid_id);

    let update_result = db
        .get_ref()
        .orders()
        .update_order_status_by_id(resp_order_id, OrderStatus::Executed)
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);
    let updated_order = db
        .get_ref()
        .orders()
        .get_order_by_id(resp_order_id)
        .await
        .unwrap();
    assert_eq!(updated_order.unwrap().status, OrderStatus::Executed);
}

fn db_bid_order(id: &str, market_id: &str, time_slot: u64) -> DbOrderSchema {
    DbOrderSchema {
        _id: id.to_string(),
        status: OrderStatus::Open,
        order: DbOrder::Bid(DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: DbOrderComponent {
                area_uuid: "area".to_string(),
                market_id: market_id.to_string(),
                time_slot,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    }
}

fn db_offer_order(id: &str, market_id: &str, time_slot: u64) -> DbOrderSchema {
    DbOrderSchema {
        _id: id.to_string(),
        status: OrderStatus::Open,
        order: DbOrder::Offer(DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: DbOrderComponent {
                area_uuid: "area".to_string(),
                market_id: market_id.to_string(),
                time_slot,
                creation_time: 1677453190,
                energy: 100.0,
                energy_rate: 10.0,
            },
        }),
    }
}

#[tokio::test]
async fn filter_orders_by_time_window_matches_component_time_slot() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);

    // Bid and Offer orders at distinct component time_slots across two markets.
    let orders = vec![
        db_bid_order("bid_100_a", "market_a", 100),
        db_offer_order("offer_200_a", "market_a", 200),
        db_bid_order("bid_300_b", "market_b", 300),
    ];
    db.get_ref()
        .orders()
        .insert_orders(orders)
        .await
        .expect("Failed to insert orders");

    // Time window [150, 250] should match only the Offer at time_slot 200
    // (covers both a Bid and an Offer document via the $or on component paths).
    let mut result = db
        .get_ref()
        .orders()
        .filter_orders(None, Some(150), Some(250))
        .await
        .expect("Failed to filter orders");
    result.sort_by(|a, b| a._id.cmp(&b._id));
    let ids: Vec<&str> = result.iter().map(|o| o._id.as_str()).collect();
    assert_eq!(ids, vec!["offer_200_a"]);

    // Open-ended window (only start_time) should match time_slots >= 250.
    let result = db
        .get_ref()
        .orders()
        .filter_orders(None, Some(250), None)
        .await
        .expect("Failed to filter orders");
    let ids: Vec<&str> = result.iter().map(|o| o._id.as_str()).collect();
    assert_eq!(ids, vec!["bid_300_b"]);

    // market_id + time window combined (the $and case on the Mongo side, both
    // predicate conjuncts in-memory): market_a within [50, 150] matches only
    // the Bid at time_slot 100, excluding the Offer at 200 in the same market.
    let result = db
        .get_ref()
        .orders()
        .filter_orders(Some("market_a".to_string()), Some(50), Some(150))
        .await
        .expect("Failed to filter orders");
    let ids: Vec<&str> = result.iter().map(|o| o._id.as_str()).collect();
    assert_eq!(ids, vec!["bid_100_a"]);
}

fn db_bid_order_at(id: &str, area_uuid: &str, market_id: &str) -> DbOrderSchema {
    let mut order = db_bid_order(id, market_id, 1);
    if let DbOrder::Bid(bid) = &mut order.order {
        bid.bid_component.area_uuid = area_uuid.to_string();
    }
    order
}

fn db_offer_order_at(id: &str, area_uuid: &str, market_id: &str) -> DbOrderSchema {
    let mut order = db_offer_order(id, market_id, 1);
    if let DbOrder::Offer(offer) = &mut order.order {
        offer.offer_component.area_uuid = area_uuid.to_string();
    }
    order
}

async fn status_of(db: &web::Data<gsy_offchain_storage::db::DatabaseWrapper>, id: &str) -> OrderStatus {
    db.get_ref()
        .orders()
        .get_order_by_id(&Bson::String(id.to_string()))
        .await
        .expect("Failed to fetch order")
        .expect("Order not found")
        .status
}

#[tokio::test]
async fn update_order_by_area_market_id_marks_matching_orders_executed() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);

    // A Bid and an Offer that both match (area_x / market_x), plus two orders
    // that fail exactly one of the two conditions (both must match together).
    let orders = vec![
        db_bid_order_at("bid_match", "area_x", "market_x"),
        db_offer_order_at("offer_match", "area_x", "market_x"),
        db_bid_order_at("bid_wrong_area", "area_y", "market_x"),
        db_offer_order_at("offer_wrong_market", "area_x", "market_y"),
    ];
    db.get_ref()
        .orders()
        .insert_orders(orders)
        .await
        .expect("Failed to insert orders");

    let ok = db
        .get_ref()
        .orders()
        .update_order_by_area_market_id("area_x".to_string(), "market_x".to_string())
        .await
        .expect("Failed to update orders");
    assert!(ok);

    // Both matching orders (Bid and Offer) flip to Executed.
    assert_eq!(status_of(&db, "bid_match").await, OrderStatus::Executed);
    assert_eq!(status_of(&db, "offer_match").await, OrderStatus::Executed);
    // Orders failing either condition stay Open.
    assert_eq!(status_of(&db, "bid_wrong_area").await, OrderStatus::Open);
    assert_eq!(status_of(&db, "offer_wrong_market").await, OrderStatus::Open);
}

#[tokio::test]
async fn update_expired_orders_expires_only_past_open_orders() {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper);

    // Open order in the past -> should expire.
    let past_open = db_bid_order("past_open", "market_a", 100);
    // Open order in the future -> should stay Open.
    let future_open = db_offer_order("future_open", "market_a", 300);
    // Already-Executed order in the past -> should stay Executed.
    let mut past_executed = db_bid_order("past_executed", "market_a", 100);
    past_executed.status = OrderStatus::Executed;

    db.get_ref()
        .orders()
        .insert_orders(vec![past_open, future_open, past_executed])
        .await
        .expect("Failed to insert orders");

    // now_time_slot = 200: only past (time_slot < 200) AND Open orders expire.
    let summary = db
        .get_ref()
        .orders()
        .update_expired_orders(200, OrderStatus::Expired)
        .await
        .expect("Failed to update expired orders");

    assert_eq!(summary.matched_count, 1);
    assert_eq!(summary.modified_count, 1);

    assert_eq!(status_of(&db, "past_open").await, OrderStatus::Expired);
    assert_eq!(status_of(&db, "future_open").await, OrderStatus::Open);
    assert_eq!(status_of(&db, "past_executed").await, OrderStatus::Executed);
}

#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let test_cases = vec![("test", "err"), ("test2", "err")];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(&format!("{}/orders", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
