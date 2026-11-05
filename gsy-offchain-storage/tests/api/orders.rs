use crate::helpers::{init_app, stop_app};
use actix_web::web;
use mongodb::bson::{doc, to_bson, Bson};
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::ewds::dto::{order_status_to_ewds, order_type_to_ewds, EwdsOrderDto};
use primitives::utils::{bytes16_to_hex, epoch_to_rfc3339};
use std::collections::HashMap;

fn make_order(order_id: &str, market_id: &str, order_type: OrderEnum) -> EwdsOrderDto {
    EwdsOrderDto {
        order_id: order_id.to_string(),
        market_id: market_id.to_string(),
        order_type: order_type_to_ewds(&order_type).to_string(),
        order_status: order_status_to_ewds(&OrderStatus::Submitted).to_string(),
        time_slot: "2026-01-01T00:00:00Z".to_string(),
        quantity: 100.0,
        price_limit: 10.0,
        energy_source_preference: None,
        energy_type: None,
        created_by: "0x00000000000000000000000000000abc".to_string(),
        creation_time: "2026-01-01T00:00:01Z".to_string(),
        updated_at: None,
        reject_reason: None,
        preferred_trading_partner: None,
        preferred_energy_rate: None,
    }
}

#[tokio::test]
async fn imports_normalize_ids_and_reject_conflicts_before_writing() {
    for route in ["orders", "orders-normalized"] {
        let app = init_app().await;
        let client = reqwest::Client::new();
        let url = format!("{}/{route}", app.address);
        let id = uuid::Uuid::from_bytes([0xab; 16]);
        let market = uuid::Uuid::from_bytes([0xcd; 16]);
        let creator = uuid::Uuid::from_bytes([0xef; 16]);
        let partner = "abcdefab-abcd-abcd-abcd-abcdefabcdef";
        let mut order = make_order(
            &id.to_string().to_uppercase(),
            &market.to_string(),
            OrderEnum::Bid,
        );
        order.created_by = creator.simple().to_string().to_uppercase();
        order.preferred_trading_partner = Some(partner.to_uppercase());
        assert_eq!(
            client
                .post(&url)
                .json(&vec![order.clone()])
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let stored = app
            .db_wrapper
            .orders()
            .get_all_orders()
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(stored.order_id, bytes16_to_hex(*id.as_bytes()));
        assert_eq!(stored.market_id, bytes16_to_hex(*market.as_bytes()));
        assert_eq!(stored.created_by, bytes16_to_hex(*creator.as_bytes()));
        assert_eq!(stored.area_uuid, stored.created_by);
        assert_eq!(
            stored
                .requirements
                .as_ref()
                .unwrap()
                .trading_partner_id
                .as_deref(),
            Some(partner)
        );
        let returned: Vec<EwdsOrderDto> = client
            .get(format!("{}/orders", app.address))
            .query(&[("market_id", market.to_string().to_uppercase())])
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            DbOrderSchema::try_from(returned[0].clone()).unwrap(),
            stored
        );

        let mut new_order = order.clone();
        new_order.order_id = bytes16_to_hex([0x12; 16]);
        let mut duplicate = new_order.clone();
        duplicate.order_id = uuid::Uuid::from_bytes([0x12; 16]).to_string();
        assert_eq!(
            client
                .post(&url)
                .json(&vec![new_order.clone(), duplicate])
                .send()
                .await
                .unwrap()
                .status(),
            409
        );

        order.order_id = stored.order_id.clone();
        order.order_status = "executed".into();
        order.quantity = 999.0;
        assert_eq!(
            client
                .post(&url)
                .json(&vec![new_order, order])
                .send()
                .await
                .unwrap()
                .status(),
            409
        );
        assert_eq!(
            app.db_wrapper.orders().get_all_orders().await.unwrap(),
            vec![stored]
        );
        stop_app(app).await;
    }
}

#[tokio::test]
async fn malformed_import_identifiers_reject_the_entire_batch() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let order = make_order(
        &bytes16_to_hex([1; 16]),
        &bytes16_to_hex([2; 16]),
        OrderEnum::Bid,
    );
    let valid = serde_json::to_value(&order).unwrap();
    for route in ["orders", "orders-normalized"] {
        for (field, value) in [
            ("orderId", "00000000-0000-0000-0000-000000000000"),
            ("orderId", "{abcdefab-abcd-abcd-abcd-abcdefabcdef}"),
            ("orderId", "urn:uuid:abcdefab-abcd-abcd-abcd-abcdefabcdef"),
            ("orderId", "0Xabcdefabcdefabcdefabcdefabcdefab"),
            ("marketId", "0x00000000000000000000000000000000"),
            ("marketId", "market-1"),
            ("createdBy", "facility-a"),
            ("createdBy", "0x0000000000000000000000000000000000000abc"),
            ("createdBy", "00000000000000000000000000000000"),
            ("createdBy", " abcdefab-abcd-abcd-abcd-abcdefabcdef"),
            ("preferredTradingPartner", ""),
            ("preferredTradingPartner", "facility-a "),
        ] {
            let mut invalid = valid.clone();
            invalid["orderId"] = serde_json::json!(bytes16_to_hex([3; 16]));
            invalid[field] = serde_json::json!(value);
            assert_eq!(
                client
                    .post(format!("{}/{route}", app.address))
                    .json(&vec![valid.clone(), invalid])
                    .send()
                    .await
                    .unwrap()
                    .status(),
                400,
                "{field}: {value}"
            );
        }
    }
    assert_eq!(
        app.db_wrapper
            .orders()
            .count_documents(doc! {})
            .await
            .unwrap(),
        0
    );
    stop_app(app).await;
}

#[tokio::test]
async fn concurrent_imports_of_equivalent_ids_do_not_overwrite() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let order = make_order(
        &bytes16_to_hex([0xab; 16]),
        &bytes16_to_hex([2; 16]),
        OrderEnum::Bid,
    );
    let mut duplicate = order.clone();
    duplicate.order_id = uuid::Uuid::from_bytes([0xab; 16])
        .to_string()
        .to_uppercase();
    duplicate.quantity = 200.0;
    let (first, second) = tokio::join!(
        client
            .post(format!("{}/orders", app.address))
            .json(&vec![order])
            .send(),
        client
            .post(format!("{}/orders-normalized", app.address))
            .json(&vec![duplicate])
            .send(),
    );
    let first = first.unwrap().status().as_u16();
    let second = second.unwrap().status().as_u16();
    assert!([(200, 409), (409, 200)].contains(&(first, second)));
    let orders = app.db_wrapper.orders().get_all_orders().await.unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(
        orders[0].energy_kWh,
        if first == 200 { 100.0 } else { 200.0 }
    );
    stop_app(app).await;
}

#[tokio::test]
async fn post_orders_persists_order_to_the_db() {
    let app = init_app().await;
    let address = app.address.clone();
    let order = make_order(
        "0x00000000000000000000000000000123",
        "0x00000000000000000000000000000456",
        OrderEnum::Bid,
    );
    let orderlist = vec![order.clone()];

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders", &address))
        .json(&orderlist)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, resp.status().as_u16());
    let response = resp.json::<HashMap<usize, Bson>>().await.unwrap();
    assert!(response.contains_key(&0));

    let db = web::Data::new(app.db_wrapper.clone());
    let saved_id = to_bson(&order.order_id).unwrap();
    let saved = db
        .get_ref()
        .orders()
        .get_order_by_id(&saved_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.order_type, OrderEnum::Bid);
    assert_eq!(saved.status, OrderStatus::Submitted);

    let update_result = db
        .get_ref()
        .orders()
        .update_order_status_by_id(&saved_id, OrderStatus::Executed, 1_100)
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);

    let updated = db
        .get_ref()
        .orders()
        .get_order_by_id(&saved_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, OrderStatus::Executed);
    assert_eq!(updated.status_updated_at, Some(1_100));
    stop_app(app).await;
}

#[tokio::test]
async fn post_normalized_order_round_trips() {
    let app = init_app().await;
    let address = app.address.clone();
    let market_id = "0x000000000000000000000000d0e50001";
    let order = make_order(
        "0x000000000000000000000000d0e50002",
        market_id,
        OrderEnum::Offer,
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders-normalized", &address))
        .json(&vec![order.clone()])
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/orders?market_id={}", &address, market_id))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<EwdsOrderDto> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].order_id, order.order_id);
    assert_eq!(returned[0].created_by, order.created_by);
    stop_app(app).await;
}

#[tokio::test]
async fn post_orders_returns_400_for_invalid_payload() {
    let app = init_app().await;
    let address = app.address.clone();

    let client = reqwest::Client::new();
    for invalid_body in ["test", "test2"] {
        let resp = client
            .post(&format!("{}/orders", &address))
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
async fn filter_orders_by_market_and_time_range() {
    let app = init_app().await;
    let address = app.address.clone();

    let market_a = "0x000000000000000000000000d0e5aaaa";
    let market_b = "0x000000000000000000000000d0e5bbbb";

    // Three orders in market_a at time_slots 10, 20, 30; one in market_b at 20.
    let mut orders = Vec::new();
    for (idx, (market, ts)) in [
        (market_a, 10u64),
        (market_a, 20),
        (market_a, 30),
        (market_b, 20),
    ]
    .iter()
    .enumerate()
    {
        let mut order = make_order(
            &format!("0x000000000000000000000000d0e5{:04}", idx),
            market,
            OrderEnum::Bid,
        );
        order.time_slot = epoch_to_rfc3339(*ts);
        orders.push(order);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders-normalized", &address))
        .json(&orders)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    // Filter by market_id only -> 3 orders in market_a.
    let db = web::Data::new(app.db_wrapper.clone());
    let by_market = db
        .get_ref()
        .orders()
        .filter_orders(Some(market_a.to_string()), None, None)
        .await
        .unwrap();
    assert_eq!(by_market.len(), 3);
    assert!(by_market.iter().all(|o| o.market_id == market_a));

    // Filter by market_id + time range [20, 30) -> only the time_slot 20 order.
    let by_range = db
        .get_ref()
        .orders()
        .filter_orders(Some(market_a.to_string()), Some(20), Some(30))
        .await
        .unwrap();
    assert_eq!(by_range.len(), 1);
    assert_eq!(by_range[0].time_slot, 20);

    // Start-only bound: time_slot >= 20 across all markets -> 3 orders.
    let start_only = db
        .get_ref()
        .orders()
        .filter_orders(None, Some(20), None)
        .await
        .unwrap();
    assert_eq!(start_only.len(), 3);

    // End-only bound: time_slot < 20 across all markets -> 1 order (time_slot 10).
    let end_only = db
        .get_ref()
        .orders()
        .filter_orders(None, None, Some(20))
        .await
        .unwrap();
    assert_eq!(end_only.len(), 1);
    assert_eq!(end_only[0].time_slot, 10);

    stop_app(app).await;
}

#[tokio::test]
async fn filter_orders_time_boundaries_are_inclusive_start_exclusive_end() {
    let app = init_app().await;
    let address = app.address.clone();

    let market = "0x000000000000000000000000d0e5cccc";

    // Orders at time_slots exactly on and around the boundaries 20 and 30.
    // Range under test will be [20, 30): 20 is included, 30 is excluded.
    let slots = [19u64, 20, 29, 30];
    let mut orders = Vec::new();
    for (idx, ts) in slots.iter().enumerate() {
        let mut order = make_order(
            &format!("0x000000000000000000000000d0e5c{:03}", idx),
            market,
            OrderEnum::Bid,
        );
        order.time_slot = epoch_to_rfc3339(*ts);
        orders.push(order);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/orders-normalized", &address))
        .json(&orders)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let db = web::Data::new(app.db_wrapper.clone());
    let orders_svc = || db.get_ref().orders();

    // [20, 30): start inclusive, end exclusive -> {20, 29}.
    let both = orders_svc()
        .filter_orders(Some(market.to_string()), Some(20), Some(30))
        .await
        .unwrap();
    let mut got: Vec<u64> = both.iter().map(|o| o.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 30 - 1]); // {20, 29}: 20 included, 30 excluded

    // Start-only, start on a boundary value: time_slot >= 20 -> {20, 29, 30}.
    let start_only = orders_svc()
        .filter_orders(Some(market.to_string()), Some(20), None)
        .await
        .unwrap();
    let mut got: Vec<u64> = start_only.iter().map(|o| o.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![20, 29, 30]); // 20 included (>=), nothing below

    // End-only, end on a boundary value: time_slot < 30 -> {19, 20, 29}.
    let end_only = orders_svc()
        .filter_orders(Some(market.to_string()), None, Some(30))
        .await
        .unwrap();
    let mut got: Vec<u64> = end_only.iter().map(|o| o.time_slot).collect();
    got.sort_unstable();
    assert_eq!(got, vec![19, 20, 29]); // 30 excluded (<)

    // Empty range: start == end -> nothing (20 fails $lt 20).
    let empty = orders_svc()
        .filter_orders(Some(market.to_string()), Some(20), Some(20))
        .await
        .unwrap();
    assert!(empty.is_empty()); // [20, 20) is empty

    // Single-slot range: [20, 21) -> exactly {20}.
    let single = orders_svc()
        .filter_orders(Some(market.to_string()), Some(20), Some(21))
        .await
        .unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].time_slot, 20);

    stop_app(app).await;
}
