use crate::helpers::{init_app, stop_app};
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Bid as InsertBid, Offer as InsertOffer, OrderComponent as InsertOrderComponent,
};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters,
};
use gsy_offchain_primitives::utils::h256_to_string;
use subxt::utils::{AccountId32, H256};

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();
    let account: AccountId32 = crate::orders::create_test_accountid();
    let market_id = H256::random();
    let area_id = H256::random();
    let area_id_2 = H256::random();

    let bid = InsertBid {
        buyer: account.clone(),
        nonce: 1,
        bid_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: area_id,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };
    let offer = InsertOffer {
        seller: account.clone(),
        nonce: 1,
        offer_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: area_id_2,
            market_id,
            time_slot: 1,
            creation_time: 1677453190,
        },
    };

    let trade_uuid = H256::random();
    let trade1 = Trade {
        seller: account.clone(),
        buyer: account.clone(),
        market_id,
        time_slot: 123456123,
        trade_uuid,
        creation_time: 123456123,
        offer,
        offer_hash: H256::random(),
        bid,
        bid_hash: H256::random(),
        residual_offer: None,
        residual_bid: None,
        parameters: InsertTradeParameters {
            selected_energy: 14,
            energy_rate: 3,
            trade_uuid,
        },
    };

    let tradelist = vec![trade1.clone()];
    let body = Vec::<Trade<AccountId32, H256>>::encode(&tradelist);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());

    let db = web::Data::new(app.db_wrapper.clone());
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();

    let result_trade = saved.first().unwrap();
    assert_eq!(result_trade.trade_id, h256_to_string(trade1.trade_uuid));
    assert_eq!(result_trade.trade_status, TradeStatus::Settled);
    stop_app(app).await;
}

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
