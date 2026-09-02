use crate::helpers::{init_app, stop_app};
use actix_web::web;
use mongodb::bson::{to_bson, Bson};
use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};
use primitives::ewds::dto::{order_status_to_ewds, order_type_to_ewds, EwdsOrderDto};
use std::collections::HashMap;

fn make_order(order_id: &str, market_id: &str, order_type: OrderEnum) -> EwdsOrderDto {
    EwdsOrderDto {
        order_id: order_id.to_string(),
        market_id: market_id.to_string(),
        order_type: order_type_to_ewds(&order_type).to_string(),
        order_status: order_status_to_ewds(&OrderStatus::Open).to_string(),
        time_slot: 1,
        quantity: 100.0,
        price_limit: 10.0,
        energy_source_preference: None,
        energy_type: None,
        created_by: "0x0000000000000000000000000000000000000abc".to_string(),
        creation_time: 1_677_453_190,
        updated_at: None,
        reject_reason: None,
        preferred_trading_partner: None,
        preferred_energy_rate: None
    }
}

#[tokio::test]
async fn post_orders_persists_order_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();
    let order = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000123",
        "0x0000000000000000000000000000000000000000000000000000000000000456",
        OrderEnum::Bid,
    );
    let orderlist = vec![order.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders", &address))
        .json(&orderlist)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, resp.status().as_u16());
    let response = resp.json::<HashMap<usize, Bson>>().await.unwrap();
    assert!(response.contains_key(&0));

    let db = web::Data::new(app.db_wrapper.clone());
    let saved_id = to_bson(&order.order_id).unwrap();
    let saved = db
        .get_ref()
        .orders()
        .get_order_by_id(&saved_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.order_type, OrderEnum::Bid);
    assert_eq!(saved.status, OrderStatus::Open);

    let update_result = db
        .get_ref()
        .orders()
        .update_order_status_by_id(&saved_id, OrderStatus::Executed)
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);

    let updated = db
        .get_ref()
        .orders()
        .get_order_by_id(&saved_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, OrderStatus::Executed);
    stop_app(app).await;
}

#[tokio::test]
async fn post_normalized_order_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();
    let market_id = "0x00000000000000000000000000000000000000000000000000000000d0e50001";
    let order = make_order(
        "0x00000000000000000000000000000000000000000000000000000000d0e50002",
        market_id,
        OrderEnum::Offer,
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders-normalized", &address))
        .json(&vec![order.clone()])
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/orders?market_id={}", &address, market_id))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsOrderDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].order_id, order.order_id);
    assert_eq!(returned[0].created_by, order.created_by);
    stop_app(app).await;
}

#[tokio::test]
async fn post_orders_returns_400_for_invalid_payload() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    for invalid_body in ["test", "test2"] {
        let resp = client
            .post(&format!("{}/orders", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(400, resp.status().as_u16());
    }
    stop_app(app).await;
}
