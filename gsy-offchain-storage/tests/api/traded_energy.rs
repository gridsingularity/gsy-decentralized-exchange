use crate::helpers::init_app;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use serde::Deserialize;
use subxt::utils::H256;

#[derive(Deserialize, Debug, PartialEq)]
struct TimeSeriesPoint {
    timestamp: u64,
    value: f64,
}

#[derive(Deserialize, Debug)]
struct TradedEnergyResponse {
    id: String,
    traded_energy: Vec<TimeSeriesPoint>,
}

fn create_test_order_component(area_uuid: &str, time_slot: u64) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area_uuid.to_string(),
        market_id: "test-market".to_string(),
        time_slot,
        creation_time: 1677453190,
        energy: 100.0,
        energy_rate: 10.0,
    }
}

fn create_test_trade(
    bid_area_uuid: &str,
    offer_area_uuid: &str,
    time_slot: u64,
    selected_energy: f64,
) -> TradeSchema {
    let trade_uuid = H256::random().to_string();
    TradeSchema {
        _id: H256::random().to_string(),
        status: TradeStatus::Settled,
        seller: "seller".to_string(),
        buyer: "buyer".to_string(),
        market_id: "test-market".to_string(),
        time_slot,
        trade_uuid: trade_uuid.clone(),
        creation_time: 1677453190,
        status_updated_at: None,
        offer: DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: create_test_order_component(offer_area_uuid, time_slot),
        },
        offer_hash: H256::random().to_string(),
        bid: DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: create_test_order_component(bid_area_uuid, time_slot),
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

async fn post_test_trades(address: &str, trades: Vec<TradeSchema>) {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", address))
        .header("Content-Type", "application/json")
        .json(&trades)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());
}

#[tokio::test]
async fn get_traded_energy_sums_trades_for_area_on_both_sides() {
    let app = init_app().await;
    let address = app.address;

    let trades = vec![
        // Area on the bid side, two trades in the same time_slot.
        create_test_trade("area-1", "area-2", 100, 5.0),
        create_test_trade("area-1", "area-2", 100, 2.5),
        // Same area on the offer side in another time_slot.
        create_test_trade("area-3", "area-1", 200, 3.0),
        // Unrelated area, must not be counted.
        create_test_trade("area-4", "area-5", 100, 50.0),
    ];
    post_test_trades(&address, trades).await;

    let resp = reqwest::get(format!("{}/traded-energy?id=area-1", &address))
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let body: TradedEnergyResponse = resp.json().await.unwrap();
    assert_eq!(body.id, "area-1");
    assert_eq!(
        body.traded_energy,
        vec![
            TimeSeriesPoint {
                timestamp: 100,
                value: 7.5
            },
            TimeSeriesPoint {
                timestamp: 200,
                value: 3.0
            },
        ]
    );
}

#[tokio::test]
async fn get_traded_energy_applies_time_window() {
    let app = init_app().await;
    let address = app.address;

    let trades = vec![
        create_test_trade("area-1", "area-2", 100, 5.0),
        create_test_trade("area-3", "area-1", 200, 3.0),
        create_test_trade("area-1", "area-2", 300, 4.0),
    ];
    post_test_trades(&address, trades).await;

    // Only the middle time_slot falls within the window.
    let resp = reqwest::get(format!(
        "{}/traded-energy?id=area-1&start_time=150&end_time=250",
        &address
    ))
    .await
    .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let body: TradedEnergyResponse = resp.json().await.unwrap();
    assert_eq!(
        body.traded_energy,
        vec![TimeSeriesPoint {
            timestamp: 200,
            value: 3.0
        }]
    );

    // Open-ended window: everything from 200 onwards.
    let resp = reqwest::get(format!(
        "{}/traded-energy?id=area-1&start_time=200",
        &address
    ))
    .await
    .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let body: TradedEnergyResponse = resp.json().await.unwrap();
    assert_eq!(
        body.traded_energy,
        vec![
            TimeSeriesPoint {
                timestamp: 200,
                value: 3.0
            },
            TimeSeriesPoint {
                timestamp: 300,
                value: 4.0
            },
        ]
    );
}

#[tokio::test]
async fn get_traded_energy_returns_400_when_id_is_missing() {
    let app = init_app().await;
    let address = app.address;

    let resp = reqwest::get(format!("{}/traded-energy", &address))
        .await
        .expect("Failed to execute request.");
    assert_eq!(400, resp.status().as_u16());
}

#[tokio::test]
async fn get_traded_energy_returns_empty_series_for_unknown_area() {
    let app = init_app().await;
    let address = app.address;

    post_test_trades(&address, vec![create_test_trade("area-1", "area-2", 100, 5.0)]).await;

    let resp = reqwest::get(format!("{}/traded-energy?id=unknown-area", &address))
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());

    let body: TradedEnergyResponse = resp.json().await.unwrap();
    assert_eq!(body.id, "unknown-area");
    assert!(body.traded_energy.is_empty());
}
