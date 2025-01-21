use crate::helpers::init_app;
use actix_web::web;
use gsy_orderbook_service::db::schema::{Bid, Order, OrderComponent, OrderStatus};
use mongodb::bson::Bson;
use std::collections::HashMap;

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    let app = init_app().await;
    let address = app.address;

    let bid = Bid {
        buyer: "Gigi".to_string(),
        nonce: 1,
        bid_component: OrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: 1,
            market_uuid: 1,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let body = vec![Order::Bid(bid.clone())];

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
    let order_id = response.get(&0).unwrap();
    let saved = db
        .get_ref()
        .orders()
        .get_order_by_id(order_id)
        .await
        .unwrap();
    assert_eq!(200, status.as_u16());
    assert_eq!(saved.unwrap().order, Order::Bid(bid));
    let update_result = db
        .get_ref()
        .orders()
        .update_order_status_by_id(order_id, OrderStatus::Executed)
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);
    let updated_order = db
        .get_ref()
        .orders()
        .get_order_by_id(order_id)
        .await
        .unwrap();
    assert_eq!(updated_order.unwrap().status, OrderStatus::Executed);
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
