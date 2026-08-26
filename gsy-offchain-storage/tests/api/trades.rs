use crate::helpers::{init_app, stop_app};
use actix_web::web;
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::ewds::dto::EwdsTradeDto;

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

fn make_trade(trade_uuid: &str, bid: DbOrderSchema, offer: DbOrderSchema) -> EwdsTradeDto {
    EwdsTradeDto {
        trade_id: trade_uuid.to_string(),
        market_id: bid.market_id.clone(),
        bid_id: bid.order_id.clone(),
        buyer_id: bid.created_by.clone(),
        residual_bid_id: None,
        offer_id: offer.order_id.clone(),
        seller_id: offer.created_by.clone(),
        residual_offer_id: None,
        trade_status: "executed".to_string(),
        trade_quantity: 14.0,
        trade_price: 3.0,
        timestamp: 1_677_453_191,
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
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-IE-20260328-0001");
    assert_eq!(returned[0].bid_id, trade.bid_id);
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

#[tokio::test]
async fn get_trades_filters_by_time_range() {
    let app = init_app().await;
    let address = app.address.clone();
    let bid = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000b3d",
        OrderEnum::Bid,
    );
    let offer = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000a7c",
        OrderEnum::Offer,
    );
    let mut trade = make_trade("TRADE-FILTER-0001", bid, offer);
    trade.timestamp = 1_677_453_191;

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
        .json(&vec![trade.clone()])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    // Range that includes the trade timestamp
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[("start_time", "1677453190"), ("end_time", "1677453192")])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-FILTER-0001");

    // Range that excludes the trade timestamp
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[("start_time", "1677453192"), ("end_time", "1677453200")])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 0);

    stop_app(app).await;
}

#[tokio::test]
async fn get_trades_returns_all_when_no_params() {
    let app = init_app().await;
    let address = app.address.clone();
    let bid = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000b4d",
        OrderEnum::Bid,
    );
    let offer = make_order(
        "0x0000000000000000000000000000000000000000000000000000000000000a8c",
        OrderEnum::Offer,
    );
    let trade = make_trade("TRADE-NOPARAMS-0001", bid, offer);

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades", &address))
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
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-NOPARAMS-0001");

    stop_app(app).await;
}

#[tokio::test]
async fn get_trades_returns_400_when_start_after_end() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[("start_time", "1677453200"), ("end_time", "1677453190")])
        .send()
        .await
        .unwrap();

    assert_eq!(400, resp.status().as_u16());

    stop_app(app).await;
}

#[tokio::test]
async fn filter_trades_time_boundaries_are_inclusive_start_exclusive_end() {
    let app = init_app().await;
    let address = app.address.clone();

    // Seed four trades at timestamps 19, 20, 29, 30 around the [20, 30) borders.
    let slots = [19u64, 20, 29, 30];
    let client = reqwest::Client::new();
    for (idx, ts) in slots.iter().enumerate() {
        let bid = make_order(
            &format!(
                "0x00000000000000000000000000000000000000000000000000000000d0e5b{:03}",
                idx
            ),
            OrderEnum::Bid,
        );
        let offer = make_order(
            &format!(
                "0x00000000000000000000000000000000000000000000000000000000d0e5a{:03}",
                idx
            ),
            OrderEnum::Offer,
        );
        let mut trade = make_trade(&format!("TRADE-BORDER-{:04}", idx), bid, offer);
        trade.timestamp = *ts;

        let resp = client
            .post(&format!("{}/trades", &address))
            .json(&vec![trade])
            .send()
            .await
            .unwrap();
        assert_eq!(200, resp.status().as_u16());
    }

    let db = web::Data::new(app.db_wrapper.clone());
    let trades_svc = || db.get_ref().trades();

    // [20, 30): start inclusive, end exclusive -> {20, 29}.
    let both = trades_svc().filter_trades(Some(20), Some(30)).await.unwrap();
    let mut got: Vec<u64> = both.iter().map(|t| t.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 29]); // 20 included ($gte), 30 excluded ($lt)

    // Start-only, start on a boundary value: time_slot >= 20 -> {20, 29, 30}.
    let start_only = trades_svc().filter_trades(Some(20), None).await.unwrap();
    let mut got: Vec<u64> = start_only.iter().map(|t| t.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 29, 30]); // 20 included, nothing below

    // End-only, end on a boundary value: time_slot < 30 -> {19, 20, 29}.
    let end_only = trades_svc().filter_trades(None, Some(30)).await.unwrap();
    let mut got: Vec<u64> = end_only.iter().map(|t| t.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![19, 20, 29]); // 30 excluded ($lt)

    // Empty range: start == end -> nothing (20 fails $lt 20).
    let empty = trades_svc().filter_trades(Some(20), Some(20)).await.unwrap();
    assert!(empty.is_empty()); // [20, 20) is empty

    // Single-slot range: [20, 21) -> exactly {20}.
    let single = trades_svc().filter_trades(Some(20), Some(21)).await.unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].time_slot, 20);

    // No bounds: returns everything seeded here.
    let all = trades_svc().filter_trades(None, None).await.unwrap();
    assert!(all.len() >= 4);

    stop_app(app).await;
}