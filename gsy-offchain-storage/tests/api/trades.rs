use crate::helpers::{init_app, stop_app};
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid as InsertBid, Offer as InsertOffer, OrderComponent as InsertOrderComponent,
};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters,
};


#[tokio::test]
async fn post_normalized_trade_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();

    let trade = TradeSchema {
        trade_id: "TRADE-IE-20260328-0001".to_string(),
        trade_quantity: 2.5,
        trade_price: 0.21,
        trade_timestamp: "2026-03-27T18:05:30Z".to_string(),
        time_slot: "2026-03-28T10:00:00Z".to_string(),
        market_id: "DEX-SPOT-0001".to_string(),
        trade_status: TradeStatus::Executed,
        buyer: "ACTOR-IE-0007".to_string(),
        seller: "ACTOR-IE-0011".to_string(),
        bid_id: "ORDER-IE-0001".to_string(),
        offer_id: "ORDER-IE-0002".to_string(),
        residual_bid_id: Some("ORDER-IE-0003".to_string()),
        residual_offer_id: Some("ORDER-IE-0004".to_string()),
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", &address))
        .json(&vec![trade.clone()])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client.get(&format!("{}/trades", &address)).send().await.unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<TradeSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-IE-20260328-0001");
    assert_eq!(returned[0].bid_id, "ORDER-IE-0001");
    stop_app(app).await;
}

#[tokio::test]
async fn post_trades_returns_400_for_invalid_payload() {
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
