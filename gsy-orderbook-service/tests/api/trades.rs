use crate::helpers::{init_app, stop_app};
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid as InsertBid, Offer as InsertOffer, OrderComponent as InsertOrderComponent,
};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters,
};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeParameters, TradeStatus};
use gsy_offchain_primitives::utils::h256_to_string;
use subxt::utils::{AccountId32, H256};

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();
    let market_id = "market_id".to_string();
    let area_id = "area_id".to_string();
    let area_id_2 = "area_id_2".to_string();

    let bid = DbOrderSchema {
        status: OrderStatus::Open,
        order_id: "bid_id".to_string(),
        order_type: OrderEnum::Bid,
        created_by: "buyer".to_string(),
        energy_kWh: 100.,
        energy_rate: 10.,
        area_uuid: area_id.clone(),
        market_id: market_id.clone(),
        time_slot: 1,
        creation_time: 1677453190,
        requirements: None,
        attributes: None
    };
    let offer = DbOrderSchema {
        status: OrderStatus::Open,
        order_id: "offer_id".to_string(),
        order_type: OrderEnum::Offer,
        created_by: "seller".to_string(),
        energy_kWh: 100.,
        energy_rate: 10.,
        area_uuid: area_id_2.clone(),
        market_id: market_id.clone(),
        time_slot: 1,
        creation_time: 1677453190,
        requirements: None,
        attributes: None
    };

    let trade1 = TradeSchema {
        status: TradeStatus::Executed,
        trade_uuid: "trade_id".to_string(),
        offer_hash: "offer_hash".to_string(),
        bid_hash: "bid_hash".to_string(),
        seller: "seller".to_string(),
        buyer: "buyer".to_string(),
        market_id: market_id.clone(),
        time_slot: 123456123,
        creation_time: 123456123,
        offer,
        bid,
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy_kWh: 14.,
            energy_rate: 3.,
        },
    };

    let tradelist = vec![trade1.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&tradelist)
        .send()
        .await;

    let status = resp.unwrap().status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper.clone());
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();

    let result_trade = saved.first().unwrap();
    assert_eq!(result_trade.trade_uuid, "trade_id".to_string());
    stop_app(app).await;
}

#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
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
