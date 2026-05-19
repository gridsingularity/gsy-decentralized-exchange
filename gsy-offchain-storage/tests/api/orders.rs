use crate::helpers::{init_app, stop_app};
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, OrderStatus, OrderType,
};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid, Order, OrderComponent, OrderSchema,
};
use mongodb::bson::Bson;
use std::collections::HashMap;
use subxt::utils::{AccountId32, H256};

pub fn create_test_accountid() -> AccountId32 {
    let account_id_bytes = [
        0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];

    AccountId32::from(account_id_bytes)
}

#[tokio::test]
async fn post_orders_persists_order_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();

    let account: AccountId32 = create_test_accountid();
    let market_id = H256::random();
    let area_id = H256::random();

    let order = OrderSchema {
        _id: H256::random(),
        status: OrderStatus::Open,
        order: Order::Bid(Bid {
            buyer: account,
            nonce: 1,
            bid_component: OrderComponent {
                energy: 100,
                energy_rate: 10,
                area_uuid: area_id,
                market_id,
                time_slot: 1,
                creation_time: 1677453190,
            },
        }),
    };

    let orderlist = vec![order];
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
    assert_eq!(200, status.as_u16());
    let response = resp.json::<HashMap<usize, Bson>>().await.unwrap();
    assert!(response.contains_key(&0));

    let db = web::Data::new(app.db_wrapper.clone());
    let all_orders = db.get_ref().orders().get_all_orders().await.unwrap();
    assert_eq!(all_orders.len(), 1);
    let saved = &all_orders[0];
    assert_eq!(saved.order_type, OrderType::Bid);
    assert_eq!(saved.order_status, OrderStatus::Open);

    let update_result = db
        .get_ref()
        .orders()
        .update_order_status_by_id(&saved.order_id, OrderStatus::Executed)
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);

    let updated = db
        .get_ref()
        .orders()
        .get_order_by_id(&saved.order_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.order_status, OrderStatus::Executed);
    stop_app(app).await;
}

#[tokio::test]
async fn post_normalized_order_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();

    let order = DbOrderSchema {
        order_id: "ORDER-IE-0001".to_string(),
        order_type: OrderType::Bid,
        quantity: 2.5,
        price_limit: 0.22,
        time_slot: "2026-03-28T10:00:00Z".to_string(),
        market_id: "DEX-SPOT-0001".to_string(),
        order_status: OrderStatus::Open,
        creation_time: "2026-03-27T18:04:59Z".to_string(),
        created_by: "PARTY-IE-0007".to_string(),
        energy_source_preference: Some(vec!["GREEN".to_string(), "PV".to_string()]),
        energy_type: None,
        area_uuid: None,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders-normalized", &address))
        .json(&vec![order.clone()])
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/orders?market_id=DEX-SPOT-0001", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<DbOrderSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].order_id, "ORDER-IE-0001");
    assert_eq!(returned[0].created_by, "PARTY-IE-0007");
    stop_app(app).await;
}

#[tokio::test]
async fn post_orders_returns_400_for_invalid_payload() {
    let app = init_app().await;
    let address = app.address.clone();

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
    stop_app(app).await;
}
