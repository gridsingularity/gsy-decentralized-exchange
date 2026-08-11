use crate::helpers::{init_app, stop_app};
use actix_web::web;
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};

fn make_order(order_id: &str, order_type: OrderEnum) -> DbOrderSchema {
    DbOrderSchema {
        status: OrderStatus::Open,
        order_id: order_id.to_string(),
        order_type,
        created_by: "0x0000000000000000000000000000000000000abc".to_string(),
        energy_kWh: 100.0,
        energy_rate: 10.0,
        area_uuid: "0x0000000000000000000000000000000000000000000000000000000000000abc".to_string(),
        market_id: "0x0000000000000000000000000000000000000000000000000000000000000def".to_string(),
        time_slot: 1,
        creation_time: 1_677_453_190,
        requirements: None,
        attributes: None,
    }
}

fn make_trade(trade_uuid: &str, bid: DbOrderSchema, offer: DbOrderSchema) -> TradeSchema {
    TradeSchema {
        status: TradeStatus::Executed,
        trade_uuid: trade_uuid.to_string(),
        offer_hash: offer.order_id.clone(),
        bid_hash: bid.order_id.clone(),
        seller: offer.created_by.clone(),
        buyer: bid.created_by.clone(),
        market_id: bid.market_id.clone(),
        time_slot: 1,
        creation_time: 1_677_453_191,
        offer,
        bid,
        residual_offer_id: None,
        residual_bid_id: None,
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy_kWh: 14.0,
            energy_rate: 3.0,
        },
    }
}

#[tokio::test]
async fn post_trade_request_writes_trades_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();
    let bid = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000b1d",
        OrderEnum::Bid,
    );
    let offer = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000a5c",
        OrderEnum::Offer,
    );
    let trade = make_trade("trade_id", bid.clone(), offer.clone());

    let db = web::Data::new(app.db_wrapper.clone());
    db.get_ref()
        .orders()
        .insert_orders(vec![bid.clone(), offer.clone()])
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .json(&vec![trade.clone()])
        .send()
        .await
        .unwrap();

    assert_eq!(200, resp.status().as_u16());

    let saved = db.get_ref().trades().get_all_trades().await.unwrap();
    let result_trade = saved.first().unwrap();
    assert_eq!(result_trade.trade_uuid, "trade_id".to_string());

    let bid_bson = mongodb::bson::to_bson(&bid.order_id).unwrap();
    let bid_after = db
        .get_ref()
        .orders()
        .get_order_by_id(&bid_bson)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bid_after.status, OrderStatus::Executed);
    stop_app(app).await;
}

#[tokio::test]
async fn post_normalized_trade_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();
    let bid = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000b2d",
        OrderEnum::Bid,
    );
    let offer = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000a6c",
        OrderEnum::Offer,
    );
    let trade = make_trade("TRADE-IE-20260328-0001", bid, offer);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", &address))
        .json(&vec![trade.clone()])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/trades", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<TradeSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_uuid, "TRADE-IE-20260328-0001");
    assert_eq!(returned[0].bid_hash, trade.bid_hash);
    stop_app(app).await;
}

#[tokio::test]
async fn post_trades_returns_400_for_invalid_payload() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    for invalid_body in ["test", "test2"] {
        let resp = client
            .post(&format!("{}/trades", &address))
            .header("Content-Type", "application/json")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(400, resp.status().as_u16());
    }
    stop_app(app).await;
}
