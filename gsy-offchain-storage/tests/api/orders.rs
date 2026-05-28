use crate::helpers::{init_app, stop_app};
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, OrderStatus, OrderType,
};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid, Order, OrderComponent, OrderSchema,
};
use mongodb::bson::Bson;
use std::collections::HashMap;

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
