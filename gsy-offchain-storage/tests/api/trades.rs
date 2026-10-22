use crate::helpers::{init_app, stop_app};
use actix_web::web;
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::ewds::dto::EwdsTradeDto;
use primitives::utils::{epoch_to_rfc3339, timestamp_to_string_with_padding};

fn make_order(order_id: &str, order_type: OrderEnum) -> DbOrderSchema {
    DbOrderSchema {
        status: OrderStatus::Submitted,
        status_updated_at: None,
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
        timestamp: "2026-01-01T00:00:02Z".to_string(),
    }
}

fn make_market(market_id: &str, delivery_start_time: u64) -> MarketSchema {
    MarketSchema {
        market_id: market_id.to_string(),
        community_id: "11111111-1111-4111-8111-111111111111".to_string(),
        opening_time: timestamp_to_string_with_padding(delivery_start_time.saturating_sub(900)),
        closing_time: timestamp_to_string_with_padding(delivery_start_time),
        delivery_start_time: timestamp_to_string_with_padding(delivery_start_time),
        delivery_end_time: timestamp_to_string_with_padding(delivery_start_time + 900),
        market_type: MarketType::Spot,
        matching_algorithm: MatchingAlgorithm::PayAsBid,
        created_at: timestamp_to_string_with_padding(delivery_start_time.saturating_sub(900)),
    }
}

#[tokio::test]
async fn post_trade_requests_persist_trades_without_updating_orders() {
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
    let db = web::Data::new(app.db_wrapper.clone());
    db.get_ref()
        .orders()
        .insert_orders(vec![bid.clone(), offer.clone()])
        .await
        .unwrap();

    let client = reqwest::Client::new();
    for (index, endpoint) in ["/trades", "/trades-normalized"].into_iter().enumerate() {
        let trade = make_trade(&format!("trade_{index}"), bid.clone(), offer.clone());
        let resp = client
            .post(format!("{address}{endpoint}"))
            .json(&vec![trade.clone()])
            .send()
            .await
            .unwrap();

        assert_eq!(200, resp.status().as_u16());

        let saved = db.get_ref().trades().get_all_trades().await.unwrap();
        assert_eq!(saved.len(), index + 1);
        assert!(saved.iter().any(|stored| stored.trade_uuid == trade.trade_id));

        for order in [&bid, &offer] {
            let id = mongodb::bson::to_bson(&order.order_id).unwrap();
            let stored = db
                .get_ref()
                .orders()
                .get_order_by_id(&id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&stored, order);
        }
    }
    stop_app(app).await;
}

#[tokio::test]
async fn post_matched_trades_are_idempotent_without_updating_orders() {
    for endpoint in ["/trades", "/trades-normalized"] {
        let app = init_app().await;
        let client = reqwest::Client::new();
        let bid = make_order("bid", OrderEnum::Bid);
        let offer = make_order("offer", OrderEnum::Offer);
        app.db_wrapper
            .orders()
            .insert_orders(vec![bid.clone(), offer.clone()])
            .await
            .unwrap();
        let mut trade = make_trade("matched-trade", bid.clone(), offer.clone());
        trade.trade_status = "matched".to_string();
        let mut expected = DbTradeSchema::try_from(trade.clone()).unwrap();
        expected.status_updated_at = Some(expected.creation_time);

        // Repeated entries within a batch and later retries must both be harmless.
        for _ in 0..2 {
            let response = client
                .post(format!("{}{endpoint}", app.address))
                .json(&vec![trade.clone(), trade.clone()])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            let ids: serde_json::Value = response.json().await.unwrap();
            assert_eq!(ids.as_object().unwrap().len(), 2);
            assert_eq!(
                serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap())
                    .unwrap(),
                serde_json::json!([expected.clone()]),
            );
            for original in [&bid, &offer] {
                let stored = app
                    .db_wrapper
                    .orders()
                    .get_order_by_id(&original.order_id.clone().into())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(&stored, original);
            }
            trade.timestamp = epoch_to_rfc3339(expected.creation_time + 60);
        }
        stop_app(app).await;
    }
}

#[tokio::test]
async fn late_matched_imports_preserve_existing_trade_statuses() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    for status in ["matched", "settled", "executed", "penalized"] {
        let mut trade = make_trade(
            status,
            make_order("bid", OrderEnum::Bid),
            make_order("offer", OrderEnum::Offer),
        );
        trade.trade_status = status.to_string();
        let mut expected = DbTradeSchema::try_from(trade.clone()).unwrap();
        expected.status_updated_at = Some(expected.creation_time + 10);
        app.db_wrapper
            .trades()
            .insert_one(expected.clone())
            .await
            .unwrap();
        trade.trade_status = "matched".to_string();
        trade.timestamp = epoch_to_rfc3339(expected.creation_time + 60);
        for endpoint in ["/trades", "/trades-normalized"] {
            let response = client
                .post(format!("{}{endpoint}", app.address))
                .json(&vec![trade.clone()])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            let stored = app
                .db_wrapper
                .trades()
                .find_one(mongodb::bson::doc! {"trade_uuid": &trade.trade_id})
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                serde_json::to_value(stored).unwrap(),
                serde_json::json!(expected)
            );
        }
    }
    assert_eq!(
        app.db_wrapper
            .trades()
            .get_all_trades()
            .await
            .unwrap()
            .len(),
        4
    );
    stop_app(app).await;
}

#[tokio::test]
async fn matched_imports_reject_conflicting_details() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let mut trade = make_trade(
        "matched-trade",
        make_order("bid", OrderEnum::Bid),
        make_order("offer", OrderEnum::Offer),
    );
    trade.trade_status = "matched".to_string();
    client
        .post(format!("{}/trades", app.address))
        .json(&vec![trade.clone()])
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let before =
        serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap();
    for endpoint in ["/trades", "/trades-normalized"] {
        for field in [
            "buyerId",
            "sellerId",
            "marketId",
            "bidId",
            "offerId",
            "residualBidId",
            "residualOfferId",
            "tradeQuantity",
            "tradePrice",
        ] {
            let mut conflicting = serde_json::to_value(&trade).unwrap();
            conflicting[field] = if field == "tradeQuantity" || field == "tradePrice" {
                serde_json::json!(999.0)
            } else {
                serde_json::json!("different-id")
            };
            let response = client
                .post(format!("{}{endpoint}", app.address))
                .json(&vec![conflicting])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 500, "{endpoint}: {field}");
        }
    }
    assert_eq!(
        serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap(),
        before
    );
    stop_app(app).await;
}

#[tokio::test]
async fn mixed_trade_imports_keep_non_matched_records_insert_only() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let mut matched = make_trade(
        "matched-trade",
        make_order("bid", OrderEnum::Bid),
        make_order("offer", OrderEnum::Offer),
    );
    matched.trade_status = "matched".to_string();
    let mut executed = matched.clone();
    executed.trade_id = "executed-trade".to_string();
    executed.trade_status = "executed".to_string();
    let response = client
        .post(format!("{}/trades", app.address))
        .json(&vec![matched.clone(), executed.clone(), matched.clone()])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let ids: serde_json::Value = response.json().await.unwrap();
    assert_eq!(ids.as_object().unwrap().len(), 3);
    let before =
        serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap();
    for payload in [
        vec![executed.clone()],
        vec![matched.clone(), executed.clone()],
    ] {
        let response = client
            .post(format!("{}/trades-normalized", app.address))
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 500);
        assert_eq!(
            serde_json::to_value(app.db_wrapper.trades().get_all_trades().await.unwrap()).unwrap(),
            before
        );
    }
    let mut new_match = matched.clone();
    new_match.trade_id = "new-match".to_string();
    let mut after_failure = matched;
    after_failure.trade_id = "not-inserted".to_string();
    let response = client
        .post(format!("{}/trades", app.address))
        .json(&vec![new_match, executed, after_failure])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 500);
    let stored = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(stored.len(), 3);
    assert!(stored.iter().any(|trade| trade.trade_uuid == "new-match"));
    assert!(!stored
        .iter()
        .any(|trade| trade.trade_uuid == "not-inserted"));
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
        .query(&[("market_id", trade.market_id.as_str())])
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
    let trade = make_trade("TRADE-FILTER-0001", bid, offer);

    // The trade's market delivers at 2026-01-01T00:00:03Z.
    let db = web::Data::new(app.db_wrapper.clone());
    db.get_ref()
        .markets()
        .upsert(make_market(&trade.market_id, 1_767_225_603))
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

    // Range that includes the market delivery start
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[
            ("start_time", "2026-01-01T00:00:02Z"),
            ("end_time", "2026-01-01T00:00:04Z"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-FILTER-0001");

    // Filter by market_id
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[("market_id", trade.market_id.as_str())])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].trade_id, "TRADE-FILTER-0001");

    // Range that excludes the market delivery start
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[
            ("start_time", "2026-01-01T00:00:04Z"),
            ("end_time", "2026-01-01T00:00:10Z"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsTradeDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 0);

    stop_app(app).await;
}

#[tokio::test]
async fn get_trades_returns_400_without_market_id_or_full_range() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let queries: [&[(&str, &str)]; 4] = [
        &[],
        &[("start_time", "2026-01-01T00:00:00Z")],
        &[("end_time", "2026-01-01T00:00:00Z")],
        &[
            ("market_id", "MARKET-1"),
            ("start_time", "2026-01-01T00:00:00Z"),
        ],
    ];
    for query in queries {
        let resp = client
            .get(&format!("{}/trades", &address))
            .query(query)
            .send()
            .await
            .unwrap();
        assert_eq!(400, resp.status().as_u16(), "query: {:?}", query);
    }

    stop_app(app).await;
}

#[tokio::test]
async fn get_trades_returns_400_when_range_exceeds_one_day() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[
            ("start_time", "2026-01-01T00:00:00Z"),
            ("end_time", "2026-01-02T00:00:01Z"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(400, resp.status().as_u16());

    stop_app(app).await;
}

#[tokio::test]
async fn get_trades_returns_400_when_start_after_end() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/trades", &address))
        .query(&[
            ("start_time", "2023-02-26T23:13:20Z"),
            ("end_time", "2023-02-26T23:13:10Z"),
        ])
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

    // Seed four markets delivering at 19, 20, 29, 30 around the [20, 30) borders, one trade
    // each. The trade's creation_time equals its market's delivery start to identify it below.
    let slots = [19u64, 20, 29, 30];
    let db = web::Data::new(app.db_wrapper.clone());
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
        trade.market_id = format!("MARKET-BORDER-{:04}", idx);
        trade.timestamp = epoch_to_rfc3339(*ts);
        db.get_ref()
            .markets()
            .upsert(make_market(&trade.market_id, *ts))
            .await
            .unwrap();

        let resp = client
            .post(&format!("{}/trades", &address))
            .json(&vec![trade])
            .send()
            .await
            .unwrap();
        assert_eq!(200, resp.status().as_u16());
    }

    let trades_svc = || db.get_ref().trades();

    // [20, 30): start inclusive, end exclusive -> {20, 29}.
    let both = trades_svc()
        .filter_trades(None, Some(20), Some(30))
        .await
        .unwrap();
    let mut got: Vec<u64> = both.iter().map(|t| t.creation_time).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 29]); // 20 included ($gte), 30 excluded ($lt)

    // Start-only, start on a boundary value: delivery_start_time >= 20 -> {20, 29, 30}.
    let start_only = trades_svc()
        .filter_trades(None, Some(20), None)
        .await
        .unwrap();
    let mut got: Vec<u64> = start_only.iter().map(|t| t.creation_time).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 29, 30]); // 20 included, nothing below

    // End-only, end on a boundary value: delivery_start_time < 30 -> {19, 20, 29}.
    let end_only = trades_svc()
        .filter_trades(None, None, Some(30))
        .await
        .unwrap();
    let mut got: Vec<u64> = end_only.iter().map(|t| t.creation_time).collect();
    got.sort_unstable();
    assert_eq!(got, vec![19, 20, 29]); // 30 excluded ($lt)

    // Empty range: start == end -> nothing (20 fails $lt 20).
    let empty = trades_svc()
        .filter_trades(None, Some(20), Some(20))
        .await
        .unwrap();
    assert!(empty.is_empty()); // [20, 20) is empty

    // Single-slot range: [20, 21) -> exactly {20}.
    let single = trades_svc()
        .filter_trades(None, Some(20), Some(21))
        .await
        .unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].creation_time, 20);

    // No bounds: returns everything seeded here.
    let all = trades_svc().filter_trades(None, None, None).await.unwrap();
    assert!(all.len() >= 4);

    // market_id takes precedence: the range [20, 30) is ignored, so the trade of the market
    // delivering at 19 is returned.
    let by_market = trades_svc()
        .filter_trades(Some("MARKET-BORDER-0000".to_string()), Some(20), Some(30))
        .await
        .unwrap();
    assert_eq!(by_market.len(), 1);
    assert_eq!(by_market[0].creation_time, 19);

    stop_app(app).await;
}
