use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::orders::{Offer, Bid, Order, OrderComponent};
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus, TradeParameters};
use subxt::ext::sp_core::H256;

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
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
    let trade_uuid = H256::random();
    let trade1 = TradeSchema {
        _id: H256::random(),
        status: TradeStatus::Open,
        seller: "seller".to_string(),
        buyer: "buyer".to_string(),
        market_id: "market".to_string(),
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer: Offer,
        offer_hash: H256::random(),
        bid,
        bid_hash: H256::random(),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy: 14, 
            energy_rate: 0.3, 
            trade_uuid
        },
    }
    };

    let body = vec![trade1.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    let status = resp.unwrap().status();

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .trades()
        .get_all_trades()
        .await
        .unwrap();
    
    assert_eq!(200, status.as_u16());
    
    let result_trade = saved.first().unwrap();
    assert_eq!(*result_trade, trade1);
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
