use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::trades::{
    TradeParameters as DbTradeParameters, TradeSchema, TradeStatus,
};
use uuid::Uuid;

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address;
    let account = "0xAccount";
    let market_id = "0xMarket";
    let area_id = "0xArea1";
    let area_id_2 = "0xArea2";

    let bid = DbBid {
        buyer: account.to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            energy: 100.0,
            energy_rate: 10.0,
            area_uuid: area_id.to_string(),
            market_id: market_id.to_string(),
            time_slot: 1,
            creation_time: 1677453190,
        },
        requirements: None,
    };
    let offer = DbOffer {
        seller: account.to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            energy: 100.0,
            energy_rate: 10.0,
            area_uuid: area_id_2.to_string(),
            market_id: market_id.to_string(),
            time_slot: 1,
            creation_time: 1677453190,
        },
        attributes: None,
    };

    let trade_uuid = Uuid::new_v4().to_string();
    let trade1 = TradeSchema {
        _id: Uuid::new_v4().to_string(),
        status: TradeStatus::Settled,
        seller: account.to_string(),
        buyer: account.to_string(),
        market_id: market_id.to_string(),
        time_slot: 123456123,
        trade_uuid: trade_uuid.clone(),
        creation_time: 123456123,
        offer,
        offer_hash: "0xOfferHash".to_string(),
        bid,
        bid_hash: "0xBidHash".to_string(),
        residual_offer: None,
        residual_bid: None,
        parameters: DbTradeParameters {
            selected_energy: 14.0,
            energy_rate: 3.0,
            trade_uuid: trade_uuid.clone(),
        },
    };

    let tradelist = vec![trade1.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", &address))
        .header("Content-Type", "application/json")
        .json(&tradelist)
        .send()
        .await;

    let status = resp.unwrap().status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db.get_ref().trades().get_all_trades().await.unwrap();

    let result_trade = saved.first().unwrap();
    assert_eq!(result_trade.trade_uuid, trade1.trade_uuid.to_string());
}

#[tokio::test]
async fn subscribe_return_a_400_when_data_is_missing() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let test_cases = vec![("test", "err"), ("test2", "err")];

    for (invalid_body, error_message) in test_cases {
        let resp = client
            .post(&format!("{}/orders-normalized", &address))
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
