use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::{
    AreaTopologySchema, AssetType, MarketTopologySchema,
};
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use serde_json::Value;
use subxt::utils::H256;

const MARKET_ID: &str = "test-market";
const COMMUNITY_UUID: &str = "community_uuid";
const COMMUNITY_NAME: &str = "LugaggiaInnovationCommunity";
const SELLER_HASH: &str = "0xpv_area_hash";
const BUYER_HASH: &str = "0xmeter_area_hash";
/// A 15-minute boundary, as Annex A.5 rule 2 requires, and small enough to fit the `u32`
/// bounds of the measurement fetch.
const SLOT: u64 = 900 * 1_976_400;
/// When a trade gets validated, deliberately far from `SLOT`. The endpoint windows on this,
/// not on the delivery slot, and keeping the two far apart means a filter that confused them
/// would fail these tests rather than pass by coincidence.
const VALIDATED: u64 = SLOT + 100_000;

fn area(area_uuid: &str, name: &str, area_type: AssetType, area_hash: &str) -> AreaTopologySchema {
    AreaTopologySchema {
        area_uuid: area_uuid.to_string(),
        name: name.to_string(),
        area_type,
        area_hash: area_hash.to_string(),
    }
}

fn test_market() -> MarketTopologySchema {
    MarketTopologySchema {
        market_id: MARKET_ID.to_string(),
        community_uuid: COMMUNITY_UUID.to_string(),
        community_name: COMMUNITY_NAME.to_string(),
        time_slot: 100,
        creation_time: 100,
        community_areas: vec![
            area("pv-uuid", "LIC03PV", AssetType::PV, SELLER_HASH),
            area("meter-uuid", "LIC08SM", AssetType::SMART_METER, BUYER_HASH),
        ],
    }
}

fn order_component(area_hash: &str, time_slot: u64) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area_hash.to_string(),
        market_id: MARKET_ID.to_string(),
        time_slot,
        creation_time: 1677453190,
        energy: 100.0,
        energy_rate: 10.0,
    }
}

fn trade(
    time_slot: u64,
    selected_energy: f64,
    status: TradeStatus,
    status_updated_at: Option<u64>,
) -> TradeSchema {
    let trade_uuid = H256::random().to_string();
    TradeSchema {
        _id: H256::random().to_string(),
        status,
        seller: "seller_account".to_string(),
        buyer: "buyer_account".to_string(),
        market_id: MARKET_ID.to_string(),
        time_slot,
        trade_uuid: trade_uuid.clone(),
        // Trading precedes delivery: struck one hour before the delivery slot.
        creation_time: time_slot - 3600,
        status_updated_at,
        offer: DbOffer {
            seller: "seller_account".to_string(),
            nonce: 1,
            offer_component: order_component(SELLER_HASH, time_slot),
        },
        offer_hash: H256::random().to_string(),
        bid: DbBid {
            buyer: "buyer_account".to_string(),
            nonce: 1,
            bid_component: order_component(BUYER_HASH, time_slot),
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

/// A net-exporting production reading for the PV area, as a real PV meter reports it.
fn production_measurement(time_slot: u64, creation_time: u64) -> MeasurementSchema {
    MeasurementSchema {
        area_uuid: "LIC03PV".to_string(),
        area_hash: SELLER_HASH.to_string(),
        community_uuid: COMMUNITY_UUID.to_string(),
        time_slot,
        creation_time,
        energy_kwh: -4.0,
    }
}

async fn get_raw(address: &str, query: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(&format!(
            "{}/guarantees-of-origin-measurements{}",
            address, query
        ))
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap()
}

async fn get_certificates(address: &str, query: &str) -> Vec<Value> {
    let resp = get_raw(address, query).await;
    assert_eq!(200, resp.status().as_u16());
    resp.json().await.unwrap()
}

/// Every query needs a `start_time`; this is the "from the beginning" form.
async fn get_all_certificates(address: &str) -> Vec<Value> {
    get_certificates(address, "?start_time=0").await
}

fn recorded_at(record: &Value) -> u64 {
    record["measurement_provenance"]["measurement_recorded_at"]
        .as_u64()
        .unwrap()
}

#[tokio::test]
async fn start_time_is_mandatory() {
    let app = init_app().await;
    let resp = get_raw(&app.address, "").await;
    assert_eq!(
        400,
        resp.status().as_u16(),
        "a query with no start_time is rejected rather than scanning every trade ever settled"
    );
}

#[tokio::test]
async fn only_executed_trades_yield_certificates() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    db.get_ref()
        .trades()
        .insert_trades(vec![
            trade(SLOT, 3.0, TradeStatus::Executed, Some(VALIDATED)),
            trade(SLOT, 2.0, TradeStatus::Penalized, Some(VALIDATED)),
            trade(SLOT, 1.0, TradeStatus::Settled, None),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    let records = get_all_certificates(&address).await;

    assert_eq!(records.len(), 1, "only the Executed trade earns a certificate");
    assert_eq!(records[0]["time_and_quantity"]["energy_quantity"], 3.0);
    assert_eq!(
        records[0]["trade_and_delivery"]["trade_status_at_issuance"],
        "delivery_verified"
    );
    assert_eq!(records[0]["identity"]["record_type"], "local_origin_record");
}

/// The window bounds validation time. Both ends are inclusive, matching every other time
/// window in this service.
#[tokio::test]
async fn the_window_bounds_validation_time_inclusively_at_both_ends() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    let earlier = VALIDATED;
    let later = VALIDATED + 3600;
    db.get_ref()
        .trades()
        .insert_trades(vec![
            trade(SLOT, 3.0, TradeStatus::Executed, Some(earlier)),
            trade(SLOT, 4.0, TradeStatus::Executed, Some(later)),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    assert_eq!(get_all_certificates(&address).await.len(), 2);

    // Lower bound inclusive: querying exactly at a validation time returns that record.
    let from_later = get_certificates(&address, &format!("?start_time={}", later)).await;
    assert_eq!(from_later.len(), 1);
    assert_eq!(from_later[0]["time_and_quantity"]["energy_quantity"], 4.0);

    let after_later =
        get_certificates(&address, &format!("?start_time={}", later + 1)).await;
    assert!(after_later.is_empty());

    // Upper bound inclusive.
    let up_to_earlier = get_certificates(
        &address,
        &format!("?start_time=0&end_time={}", earlier),
    )
    .await;
    assert_eq!(up_to_earlier.len(), 1);
    assert_eq!(up_to_earlier[0]["time_and_quantity"]["energy_quantity"], 3.0);

    let both = get_certificates(
        &address,
        &format!("?start_time={}&end_time={}", earlier, later),
    )
    .await;
    assert_eq!(both.len(), 2);
}

/// The reason the window is on validation time rather than delivery time: a trade delivered
/// in an early slot but validated late is still returned by a window covering its validation,
/// where a window over delivery time would have closed over that slot long before.
#[tokio::test]
async fn a_late_validated_trade_is_returned_by_its_validation_window() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    let early_slot = SLOT;
    let late_slot = SLOT + 1800;
    // The earlier delivery is validated LAST — its metering arrived late.
    db.get_ref()
        .trades()
        .insert_trades(vec![
            trade(late_slot, 1.0, TradeStatus::Executed, Some(VALIDATED)),
            trade(early_slot, 2.0, TradeStatus::Executed, Some(VALIDATED + 3600)),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![
            production_measurement(early_slot, SLOT + 30),
            production_measurement(late_slot, SLOT + 30),
        ])
        .await
        .unwrap();

    // A consumer that has already read up to VALIDATED asks for what came after.
    let records =
        get_certificates(&address, &format!("?start_time={}", VALIDATED + 1)).await;

    assert_eq!(records.len(), 1, "the late-validated trade is returned");
    assert_eq!(
        records[0]["time_and_quantity"]["source_slot_timestamp"], early_slot,
        "even though its delivery slot is the earlier of the two"
    );
    assert_eq!(records[0]["time_and_quantity"]["energy_quantity"], 2.0);
}

/// Trades promoted before `status_updated_at` existed carry no value and cannot be windowed.
#[tokio::test]
async fn an_executed_trade_without_a_status_change_time_is_not_returned() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    db.get_ref()
        .trades()
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, None)])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    assert!(
        get_all_certificates(&address).await.is_empty(),
        "a legacy Executed trade with no status_updated_at has no window to fall in"
    );
}

/// Records come back in a stable order, so adjacent or repeated queries agree.
///
/// The sort key leads on `measurement_recorded_at`, so the measurements carry three distinct
/// arrival times, deliberately ordered differently from both the insertion order and the
/// delivery-slot order. Giving them a common arrival time would make this test vacuous — every
/// key equal, any order trivially "sorted".
#[tokio::test]
async fn certificates_are_returned_in_ascending_checkpoint_order() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    // Inserted deliberately out of order.
    db.get_ref()
        .trades()
        .insert_trades(vec![
            trade(SLOT + 900, 1.0, TradeStatus::Executed, Some(VALIDATED + 300)),
            trade(SLOT, 2.0, TradeStatus::Executed, Some(VALIDATED + 100)),
            trade(SLOT + 1800, 3.0, TradeStatus::Executed, Some(VALIDATED + 200)),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![
            production_measurement(SLOT, SLOT + 100),
            production_measurement(SLOT + 900, SLOT + 300),
            production_measurement(SLOT + 1800, SLOT + 200),
        ])
        .await
        .unwrap();

    let records = get_all_certificates(&address).await;
    assert_eq!(records.len(), 3);

    // Ascending arrival time, which here is neither the insertion nor the delivery order.
    let slots: Vec<u64> = records
        .iter()
        .map(|r| r["time_and_quantity"]["source_slot_timestamp"].as_u64().unwrap())
        .collect();
    assert_eq!(slots, vec![SLOT, SLOT + 1800, SLOT + 900]);

    let checkpoints: Vec<u64> = records.iter().map(recorded_at).collect();
    let mut sorted = checkpoints.clone();
    sorted.sort_unstable();
    assert_eq!(checkpoints, sorted, "ascending, not insertion order");
}

#[tokio::test]
async fn no_matching_trade_returns_an_empty_list() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    assert!(get_all_certificates(&address).await.is_empty());
}

#[tokio::test]
async fn unknown_market_topology_returns_an_empty_list_not_a_partial_record() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    // No market inserted: the seller area hash resolves to nothing.
    db.get_ref()
        .trades()
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, Some(VALIDATED))])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    assert!(
        get_all_certificates(&address).await.is_empty(),
        "an unresolvable trade is skipped, not emitted half-built"
    );
}

#[tokio::test]
async fn a_trade_without_a_production_measurement_yields_no_certificate() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    db.get_ref()
        .trades()
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, Some(VALIDATED))])
        .await
        .unwrap();

    assert!(
        get_all_certificates(&address).await.is_empty(),
        "seller-side evidence is required: no measurement, no certificate"
    );
}
