use crate::helpers::{init_app, stop_app};
use actix_web::web;
use primitives::db_api_schema::market::{MarketSchema, MarketType, MatchingAlgorithm};
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::ewds::dto::EwdsTradeDto;
use primitives::utils::{epoch_to_rfc3339, timestamp_to_string_with_padding};

#[tokio::test]
async fn imports_normalize_trade_ids_and_parties_for_queries_and_penalties() {
    use gsy_ethers_listener::{GsyEventHandler, PenaltyRecordedFilter};
    use gsy_offchain_storage::evm_handler::OffchainStorageEvmHandler;
    use primitives::offchain_storage::{OffchainStorageClient, OffchainStorageTransport};
    use primitives::utils::bytes16_to_hex;
    for route in ["trades", "trades-normalized"] {
        let app = init_app().await;
        let client = reqwest::Client::new();
        let uuid = |byte| {
            uuid::Uuid::from_bytes([byte; 16])
                .to_string()
                .to_uppercase()
        };
        let mut trade = make_trade(
            &uuid(0xab),
            make_order(&uuid(0xbc), OrderEnum::Bid),
            make_order(&uuid(0xcd), OrderEnum::Offer),
        );
        trade.market_id = uuid(0xde);
        trade.buyer_id = uuid(0xef);
        trade.seller_id = "0xABCDEFABCDEFABCDEFABCDEFABCDEFAB".into();
        trade.trade_status = "matched".into();
        trade.residual_bid_id = Some("00000000000000000000000000000000".into());
        trade.residual_offer_id = Some("".into());
        app.db_wrapper
            .markets()
            .upsert(make_market(&bytes16_to_hex([0xde; 16]), 1000))
            .await
            .unwrap();
        assert_eq!(
            client
                .post(format!("{}/{route}", app.address))
                .json(&vec![trade.clone()])
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let source = OffchainStorageClient::new(
            OffchainStorageTransport::Http,
            app.address.clone(),
            "UNUSED",
            "test",
        );
        let stored = source
            .fetch_trades(None, Some(1000), Some(1900))
            .await
            .unwrap();
        assert_eq!(stored.len(), 1);
        let stored = &stored[0];
        assert_eq!(stored.trade_uuid, bytes16_to_hex([0xab; 16]));
        assert_eq!(stored.market_id, bytes16_to_hex([0xde; 16]));
        assert_eq!(stored.bid_hash, bytes16_to_hex([0xbc; 16]));
        assert_eq!(stored.offer_hash, bytes16_to_hex([0xcd; 16]));
        assert_eq!(stored.buyer, bytes16_to_hex([0xef; 16]));
        assert_eq!(stored.seller, "0xabcdefabcdefabcdefabcdefabcdefab");
        assert_eq!(stored.residual_bid_id, None);
        assert_eq!(stored.residual_offer_id, None);
        for market in [&trade.market_id, &stored.market_id] {
            assert_eq!(
                source.fetch_trades(Some(market), None, None).await.unwrap(),
                vec![stored.clone()]
            );
        }
        let gateway = crate::ewds_handler::mock_gateway().await;
        let config = crate::ewds_handler::test_config(gateway.uri());
        gsy_offchain_storage::ewds_handler::handle_request(
            &app.db_wrapper,
            &client,
            &config,
            primitives::ewds::dto::EwdsRequestEnvelope {
                request_id: "mixed-trade".into(),
                operation: primitives::ewds::EwdsOperation::TradesQuery,
                payload: serde_json::json!({"marketId": trade.market_id}),
            },
        )
        .await
        .unwrap();
        let requests = gateway.received_requests().await.unwrap();
        let sent: primitives::ewds::dto::EwdsSendMessageDto =
            serde_json::from_slice(&requests[0].body).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&sent.payload).unwrap();
        assert_eq!(
            reply["data"],
            serde_json::json!([EwdsTradeDto::from(stored.clone())])
        );

        // Known conflicts must not leave the first, otherwise-new record behind.
        let mut new_trade = trade.clone();
        new_trade.trade_id = bytes16_to_hex([0xaa; 16]);
        let mut duplicate = new_trade.clone();
        duplicate.trade_id = uuid(0xaa);
        for batch in [vec![new_trade.clone(), duplicate], vec![new_trade, trade]] {
            assert_eq!(
                client
                    .post(format!("{}/{route}", app.address))
                    .json(&batch)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                409
            );
        }
        assert_eq!(
            app.db_wrapper
                .trades()
                .get_all_trades()
                .await
                .unwrap()
                .len(),
            1
        );
        OffchainStorageEvmHandler {
            db: app.db_wrapper.clone(),
            event_publisher: None,
        }
        .handle_penalty_recorded(
            PenaltyRecordedFilter {
                trade_id: [0xab; 16],
                market_id: [0xde; 16],
                penalized_actor_id: [0xef; 16],
                penalty_energy: 1,
            },
            2000,
        )
        .await
        .unwrap();
        assert_eq!(
            app.db_wrapper.trades().get_all_trades().await.unwrap()[0].status,
            primitives::db_api_schema::trades::TradeStatus::Penalized
        );
        stop_app(app).await;
    }
}

#[tokio::test]
async fn concurrent_trade_imports_keep_one_record_with_canonical_residuals() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let mut trade = make_trade(
        "ABCDEFAB-ABCD-ABCD-ABCD-ABCDEFABCDEF",
        make_order("0x11111111111111111111111111111111", OrderEnum::Bid),
        make_order("0x22222222222222222222222222222222", OrderEnum::Offer),
    );
    trade.residual_bid_id = Some("BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB".into());
    trade.residual_offer_id = Some("CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC".into());
    let mut equivalent = trade.clone();
    equivalent.trade_id = "0xabcdefababcdabcdabcdabcdefabcdef".into();
    let first = client
        .post(format!("{}/trades", app.address))
        .json(&vec![trade]);
    let second = client
        .post(format!("{}/trades-normalized", app.address))
        .json(&vec![equivalent]);
    let (first, second) = tokio::join!(first.send(), second.send());
    let mut statuses = [
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let trades = app.db_wrapper.trades().get_all_trades().await.unwrap();
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].trade_uuid, "0xabcdefababcdabcdabcdabcdefabcdef");
    assert_eq!(
        trades[0].residual_bid_id.as_deref(),
        Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    );
    assert_eq!(
        trades[0].residual_offer_id.as_deref(),
        Some("0xcccccccccccccccccccccccccccccccc")
    );
    stop_app(app).await;
}

#[tokio::test]
async fn trade_imports_validate_all_identifiers_before_writing() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let trade = make_trade(
        "abcdefab-abcd-abcd-abcd-abcdefabcdef",
        make_order("0x11111111111111111111111111111111", OrderEnum::Bid),
        make_order("0x22222222222222222222222222222222", OrderEnum::Offer),
    );
    let valid = serde_json::to_value(trade).unwrap();
    for route in ["trades", "trades-normalized"] {
        for field in [
            "tradeId",
            "marketId",
            "bidId",
            "offerId",
            "buyerId",
            "sellerId",
            "residualBidId",
            "residualOfferId",
        ] {
            for invalid in [
                " padded ",
                "0Xabcdefabcdefabcdefabcdefabcdefab",
                "not-an-id",
            ] {
                let mut bad = valid.clone();
                bad[field] = serde_json::json!(invalid);
                assert_eq!(
                    client
                        .post(format!("{}/{route}", app.address))
                        .json(&vec![valid.clone(), bad])
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    400,
                    "{field}: {invalid}"
                );
            }
            if !field.starts_with("residual") {
                let mut bad = valid.clone();
                bad[field] = serde_json::json!("00000000-0000-0000-0000-000000000000");
                assert_eq!(
                    client
                        .post(format!("{}/{route}", app.address))
                        .json(&vec![bad])
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    400
                );
            }
        }
    }
    assert!(app
        .db_wrapper
        .trades()
        .get_all_trades()
        .await
        .unwrap()
        .is_empty());
    stop_app(app).await;
}

fn make_order(order_id: &str, order_type: OrderEnum) -> DbOrderSchema {
    DbOrderSchema {
        status: OrderStatus::Submitted,
        status_updated_at: None,
        order_id: order_id.to_string(),
        order_type,
        created_by: "0x00000000000000000000000000000abc".to_string(),
        energy_kWh: 100.0,
        energy_rate: 10.0,
        area_uuid: "0x00000000000000000000000000000abc".to_string(),
        market_id: "0x00000000000000000000000000000def".to_string(),
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
    let bid = make_order("0x00000000000000000000000000000b1d", OrderEnum::Bid);
    let offer = make_order("0x00000000000000000000000000000a5c", OrderEnum::Offer);
    let db = web::Data::new(app.db_wrapper.clone());
    db.get_ref()
        .orders()
        .insert_orders(vec![bid.clone(), offer.clone()])
        .await
        .unwrap();

    let client = reqwest::Client::new();
    for (index, endpoint) in ["/trades", "/trades-normalized"].into_iter().enumerate() {
        let trade = make_trade(&format!("0x{:032x}", index + 1), bid.clone(), offer.clone());
        let resp = client
            .post(format!("{address}{endpoint}"))
            .json(&vec![trade.clone()])
            .send()
            .await
            .unwrap();

        assert_eq!(200, resp.status().as_u16());
        let inserted: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(inserted.as_object().unwrap().len(), 1);
        let object_id = inserted["0"]["$oid"].as_str().unwrap();
        mongodb::bson::oid::ObjectId::parse_str(object_id).unwrap();

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
async fn post_normalized_trade_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();
    let bid = make_order("0x00000000000000000000000000000b2d", OrderEnum::Bid);
    let offer = make_order("0x00000000000000000000000000000a6c", OrderEnum::Offer);
    let trade = make_trade("0xabcdefabcdefabcdefabcdefabcdef01", bid, offer);

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
    assert_eq!(returned[0].trade_id, "0xabcdefabcdefabcdefabcdefabcdef01");
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
    let bid = make_order("0x00000000000000000000000000000b3d", OrderEnum::Bid);
    let offer = make_order("0x00000000000000000000000000000a7c", OrderEnum::Offer);
    let trade = make_trade("0xabcdefabcdefabcdefabcdefabcdef02", bid, offer);

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
    assert_eq!(returned[0].trade_id, "0xabcdefabcdefabcdefabcdefabcdef02");

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
    assert_eq!(returned[0].trade_id, "0xabcdefabcdefabcdefabcdefabcdef02");

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
            ("market_id", "0x00000000000000000000000000000001"),
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
            &format!("0x000000000000000000000000d0e5b{:03}", idx),
            OrderEnum::Bid,
        );
        let offer = make_order(
            &format!("0x000000000000000000000000d0e5a{:03}", idx),
            OrderEnum::Offer,
        );
        let mut trade = make_trade(&format!("0x{:032x}", idx + 1), bid, offer);
        trade.market_id = format!("0x{:032x}", idx + 1);
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
        .filter_trades(Some("0x00000000000000000000000000000001".to_string()), Some(20), Some(30))
        .await
        .unwrap();
    assert_eq!(by_market.len(), 1);
    assert_eq!(by_market[0].creation_time, 19);

    stop_app(app).await;
}
