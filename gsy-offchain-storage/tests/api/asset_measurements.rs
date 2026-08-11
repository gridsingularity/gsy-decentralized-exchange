use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use serde_json::Value;
use subxt::utils::H256;

const MARKET_ID: &str = "test-market";
const COMMUNITY_UUID: &str = "community_uuid";

fn create_test_order_component(area_uuid: &str, time_slot: u64) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area_uuid.to_string(),
        market_id: MARKET_ID.to_string(),
        time_slot,
        creation_time: 1677453190,
        energy: 100.0,
        energy_rate: 10.0,
    }
}

/// Build a settled trade. `time_slot` is the delivery timestamp; `creation_time`
/// (the moment the trade was struck, ahead of delivery) is derived from it.
fn create_test_trade(
    seller: &str,
    buyer: &str,
    time_slot: u64,
    selected_energy: f64,
) -> TradeSchema {
    let trade_uuid = H256::random().to_string();
    // Trading precedes delivery: struck one hour before the delivery slot.
    let creation_time = time_slot - 3600;
    TradeSchema {
        _id: H256::random().to_string(),
        status: TradeStatus::Settled,
        seller: seller.to_string(),
        buyer: buyer.to_string(),
        market_id: MARKET_ID.to_string(),
        time_slot,
        trade_uuid: trade_uuid.clone(),
        creation_time,
        offer: DbOffer {
            seller: seller.to_string(),
            nonce: 1,
            offer_component: create_test_order_component("offer_area", time_slot),
        },
        offer_hash: H256::random().to_string(),
        bid: DbBid {
            buyer: buyer.to_string(),
            nonce: 1,
            bid_component: create_test_order_component("bid_area", time_slot),
        },
        bid_hash: H256::random().to_string(),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy,
            energy_rate: 3.0,
            trade_uuid,
        },
    }
}

fn test_market() -> MarketTopologySchema {
    MarketTopologySchema {
        market_id: MARKET_ID.to_string(),
        community_uuid: COMMUNITY_UUID.to_string(),
        community_name: "community".to_string(),
        time_slot: 100,
        creation_time: 100,
        community_areas: vec![],
    }
}

async fn post_trades(address: &str, trades: &[TradeSchema]) {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", address))
        .header("Content-Type", "application/json")
        .json(&trades)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
}

async fn get_guarantees_of_origin(address: &str, query: &str) -> Vec<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!(
            "{}/guarantees-of-origin-measurements{}",
            address, query
        ))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    resp.json().await.unwrap()
}

#[tokio::test]
async fn get_guarantees_of_origin_reports_trade_fields() {
    let app = init_app().await;
    let address = app.address;

    let db = web::Data::new(app.db_wrapper);
    db.get_ref()
        .markets()
        .insert(test_market())
        .await
        .expect("Failed to insert market");

    let trade = create_test_trade("seller_account", "buyer_account", 4000, 5.5);
    let trade_uuid = trade.trade_uuid.clone();
    let creation_time = trade.creation_time;
    post_trades(&address, &[trade]).await;

    let resp_json = get_guarantees_of_origin(&address, "").await;

    assert_eq!(resp_json.len(), 1);
    let item = resp_json[0].as_object().unwrap();

    // Exactly the guarantees-of-origin fields are present.
    let expected_keys = [
        "trade_id",
        "traded_energy_kwh",
        "buyer_id",
        "seller_id",
        "energy_community_id",
        "energy_delivery_timestamp",
        "energy_trade_timestamp",
        "market_id",
        "market_type",
    ];
    assert_eq!(item.len(), expected_keys.len());
    for key in expected_keys {
        assert!(item.contains_key(key), "missing field {key}");
    }

    assert_eq!(item["trade_id"], trade_uuid);
    assert_eq!(item["traded_energy_kwh"], 5.5);
    assert_eq!(item["buyer_id"], "buyer_account");
    assert_eq!(item["seller_id"], "seller_account");
    assert_eq!(item["energy_community_id"], COMMUNITY_UUID);
    assert_eq!(item["energy_delivery_timestamp"], 4000);
    assert_eq!(item["energy_trade_timestamp"], creation_time);
    // Trading precedes delivery.
    assert!(item["energy_trade_timestamp"].as_u64().unwrap() < 4000);
    assert_eq!(item["market_id"], MARKET_ID);
    assert_eq!(item["market_type"], "Spot");
}

#[tokio::test]
async fn get_guarantees_of_origin_leaves_community_empty_for_unknown_market() {
    let app = init_app().await;
    let address = app.address;

    // No market topology inserted, so the community id cannot be resolved.
    post_trades(
        &address,
        &[create_test_trade("seller_account", "buyer_account", 4000, 1.0)],
    )
    .await;

    let resp_json = get_guarantees_of_origin(&address, "").await;
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0]["energy_community_id"], "");
}

#[tokio::test]
async fn get_guarantees_of_origin_filters_by_delivery_time_window() {
    let app = init_app().await;
    let address = app.address;

    post_trades(
        &address,
        &[
            create_test_trade("seller_account", "buyer_account", 4000, 1.0),
            create_test_trade("seller_account", "buyer_account", 8000, 2.0),
            create_test_trade("seller_account", "buyer_account", 12000, 3.0),
        ],
    )
    .await;

    // Lower bound only (inclusive).
    let resp_json = get_guarantees_of_origin(&address, "?start_time=8000").await;
    assert_eq!(resp_json.len(), 2);
    assert!(
        resp_json
            .iter()
            .all(|item| item["energy_delivery_timestamp"] != 4000)
    );

    // Upper bound only (inclusive).
    let resp_json = get_guarantees_of_origin(&address, "?end_time=8000").await;
    assert_eq!(resp_json.len(), 2);
    assert!(
        resp_json
            .iter()
            .all(|item| item["energy_delivery_timestamp"] != 12000)
    );

    // Both bounds: only the middle delivery slot remains.
    let resp_json = get_guarantees_of_origin(&address, "?start_time=6000&end_time=10000").await;
    assert_eq!(resp_json.len(), 1);
    assert_eq!(resp_json[0]["energy_delivery_timestamp"], 8000);
    assert_eq!(resp_json[0]["traded_energy_kwh"], 2.0);
}
