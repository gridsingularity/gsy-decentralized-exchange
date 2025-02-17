use crate::helpers::init_app;
use actix_web::web;
use codec::Encode;
use gsy_offchain_primitives::node_to_api_schema::insert_order::{
    Offer as InsertOffer, Bid as InsertBid, OrderComponent as InsertOrderComponent};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::{
    Trade, TradeParameters as InsertTradeParameters};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::AccountId32;

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address;
    let account: AccountId32 = crate::orders::create_test_accountid();
    let market_id = H256::random();

    let bid = InsertBid {
        buyer: account.clone(),
        nonce: 1,
        bid_component: InsertOrderComponent {
            energy: 100,
            energy_rate: 10,
            area_uuid: 1,
            market_id: market_id.clone(),
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
            area_uuid: 1,
            market_id: market_id.clone(),
            time_slot: 1,
            creation_time: 1677453190,
        },
    };

    let trade_uuid = H256::random();
    let trade1 = Trade {
        _id: H256::random(),
        seller: account.clone(),
        buyer: account.clone(),
        market_id: market_id.clone(),
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
            trade_uuid
        }
    };

    let tradelist = vec![trade1.clone()];
    let body = Vec::<Trade<AccountId32, H256>>::encode(&tradelist);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    let status = resp.unwrap().status();
    assert_eq!(200, status.as_u16());

    let db = web::Data::new(app.db_wrapper);
    let saved = db
        .get_ref()
        .trades()
        .get_all_trades()
        .await
        .unwrap();

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
