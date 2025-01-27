use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::orders::OrderStatus;
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Order, OrderComponent, Bid, OrderSchema};
use mongodb::bson::Bson;
use std::collections::HashMap;
use codec::Encode;
use subxt::ext::sp_core::crypto::AccountId32;
use subxt::utils::H256;

pub fn create_test_accountid() -> AccountId32 {
    // A fixed 32-byte value, typically derived from a public key
    let account_id_bytes = [
        0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31
    ];

    AccountId32::from(account_id_bytes)
}

#[tokio::test]
async fn subscribe_return_a_200_for_valid_form_data() {
    let app = init_app().await;
    let address = app.address;

    let account: AccountId32 = create_test_accountid();
    let order_id = H256::random();

    let order = OrderSchema {
        _id: order_id,
        status: OrderStatus::Expired,
        order: Order::Bid(Bid {
            buyer: account,
            nonce: 1,
            bid_component: OrderComponent {
                energy: 100,
                energy_rate: 10,
                area_uuid: 1,
                market_uuid: 1,
                time_slot: 1,
                creation_time: 1677453190,
            }
        })
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
    assert_eq!(resp_order_id.as_str().unwrap().to_string(), order_id.to_string());
    let saved = db
        .get_ref()
        .orders()
        .get_order_by_id(resp_order_id)
        .await
        .unwrap();

    assert_eq!(200, status.as_u16());
    assert_eq!(saved.unwrap()._id, order._id.to_string());

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
