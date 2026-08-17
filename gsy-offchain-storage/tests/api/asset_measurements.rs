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
/// bounds of the delivery-time filter.
const SLOT: u64 = 900 * 1_976_400;

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

async fn get_certificates(address: &str, query: &str) -> Vec<Value> {
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

fn recorded_at(record: &Value) -> u64 {
    record["measurement_provenance"]["measurement_recorded_at"]
        .as_u64()
        .unwrap()
}

fn trade_ref(record: &Value) -> String {
    record["trade_and_delivery"]["trade_reference"][0]
        .as_str()
        .unwrap()
        .to_string()
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
            trade(SLOT, 3.0, TradeStatus::Executed, Some(SLOT + 60)),
            trade(SLOT, 2.0, TradeStatus::Penalized, Some(SLOT + 60)),
            trade(SLOT, 1.0, TradeStatus::Settled, None),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    let records = get_certificates(&address, "").await;

    assert_eq!(records.len(), 1, "only the Executed trade earns a certificate");
    assert_eq!(records[0]["time_and_quantity"]["energy_quantity"], 3.0);
    assert_eq!(
        records[0]["trade_and_delivery"]["trade_status_at_issuance"],
        "delivery_verified"
    );
    assert_eq!(records[0]["identity"]["record_type"], "local_origin_record");
}

#[tokio::test]
async fn delivery_time_window_bounds_interval_start_inclusively() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    let earlier = SLOT;
    let later = SLOT + 900;
    db.get_ref()
        .trades()
        .insert_trades(vec![
            trade(earlier, 3.0, TradeStatus::Executed, Some(SLOT + 60)),
            trade(later, 4.0, TradeStatus::Executed, Some(SLOT + 60)),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![
            production_measurement(earlier, SLOT + 30),
            production_measurement(later, SLOT + 30),
        ])
        .await
        .unwrap();

    let all = get_certificates(&address, "").await;
    assert_eq!(all.len(), 2);

    // Both bounds are inclusive, so each single-slot window returns exactly its own slot.
    let from_later = get_certificates(&address, &format!("?start_time={}", later)).await;
    assert_eq!(from_later.len(), 1);
    assert_eq!(from_later[0]["time_and_quantity"]["source_slot_timestamp"], later);

    let up_to_earlier = get_certificates(&address, &format!("?end_time={}", earlier)).await;
    assert_eq!(up_to_earlier.len(), 1);
    assert_eq!(
        up_to_earlier[0]["time_and_quantity"]["source_slot_timestamp"],
        earlier
    );

    let both = get_certificates(
        &address,
        &format!("?start_time={}&end_time={}", earlier, later),
    )
    .await;
    assert_eq!(both.len(), 2);
}

#[tokio::test]
async fn recorded_after_is_exclusive_and_recorded_before_is_inclusive() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    db.get_ref()
        .trades()
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, Some(SLOT + 60))])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    let all = get_certificates(&address, "").await;
    assert_eq!(all.len(), 1);
    let checkpoint = recorded_at(&all[0]);

    let after = get_certificates(&address, &format!("?recorded_after={}", checkpoint)).await;
    assert!(
        after.is_empty(),
        "recorded_after is exclusive, so the checkpoint record is not returned again"
    );

    let after_one_less =
        get_certificates(&address, &format!("?recorded_after={}", checkpoint - 1)).await;
    assert_eq!(after_one_less.len(), 1);

    let before = get_certificates(&address, &format!("?recorded_before={}", checkpoint)).await;
    assert_eq!(before.len(), 1, "recorded_before is inclusive");

    let before_one_less =
        get_certificates(&address, &format!("?recorded_before={}", checkpoint - 1)).await;
    assert!(before_one_less.is_empty());
}

/// The incremental-consumption contract of §4, exercised the way a consumer actually runs:
/// poll, let more data arrive, poll again on the checkpoint. Every certificate must be
/// delivered exactly once — including a late arrival whose delivery slot precedes a window
/// the consumer has already read past. Polling on `start_time`/`end_time` alone would lose
/// that record, which is why the checkpoint keys on arrival rather than delivery.
#[tokio::test]
async fn polling_on_recorded_after_sees_every_certificate_exactly_once() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    let recent_slot = SLOT + 1800;
    let late_slot = SLOT;

    // Round 1: one trade, promoted early, for the later delivery slot.
    let first = trade(recent_slot, 2.0, TradeStatus::Executed, Some(SLOT + 100));
    let first_uuid = first.trade_uuid.clone();
    db.get_ref().trades().insert_trades(vec![first]).await.unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(recent_slot, SLOT + 10)])
        .await
        .unwrap();

    let mut seen: Vec<String> = Vec::new();
    let page = get_certificates(&address, "").await;
    assert_eq!(page.len(), 1);
    let mut checkpoint = recorded_at(page.last().unwrap());
    seen.push(trade_ref(&page[0]));

    // Round 2: metering for an EARLIER delivery slot finally arrives and its trade is
    // promoted now, after the consumer has already read past that delivery window.
    let late = trade(late_slot, 3.0, TradeStatus::Executed, Some(SLOT + 300));
    let late_uuid = late.trade_uuid.clone();
    db.get_ref().trades().insert_trades(vec![late]).await.unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(late_slot, SLOT + 10)])
        .await
        .unwrap();

    let page = get_certificates(&address, &format!("?recorded_after={}", checkpoint)).await;
    assert_eq!(
        page.len(),
        1,
        "the late arrival is picked up even though its delivery slot is already past"
    );
    assert_eq!(trade_ref(&page[0]), late_uuid);
    assert_eq!(page[0]["time_and_quantity"]["source_slot_timestamp"], late_slot);
    checkpoint = recorded_at(page.last().unwrap());
    seen.push(trade_ref(&page[0]));

    // Round 3: nothing new.
    let page = get_certificates(&address, &format!("?recorded_after={}", checkpoint)).await;
    assert!(page.is_empty(), "a caught-up consumer receives nothing");

    assert_eq!(seen, vec![first_uuid, late_uuid], "each delivered exactly once");
}

/// Records come back ascending by `measurement_recorded_at`. A consumer takes the last
/// record's value as its next checkpoint, so an unsorted response would let it skip
/// everything ordered after the one that happened to land last.
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
            trade(SLOT + 900, 1.0, TradeStatus::Executed, Some(SLOT + 300)),
            trade(SLOT, 2.0, TradeStatus::Executed, Some(SLOT + 100)),
            trade(SLOT + 1800, 3.0, TradeStatus::Executed, Some(SLOT + 200)),
        ])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![
            production_measurement(SLOT, SLOT + 10),
            production_measurement(SLOT + 900, SLOT + 10),
            production_measurement(SLOT + 1800, SLOT + 10),
        ])
        .await
        .unwrap();

    let records = get_certificates(&address, "").await;
    assert_eq!(records.len(), 3);

    let checkpoints: Vec<u64> = records.iter().map(recorded_at).collect();
    assert_eq!(
        checkpoints,
        vec![SLOT + 100, SLOT + 200, SLOT + 300],
        "ascending by measurement_recorded_at, not by insertion order"
    );
    assert_eq!(
        checkpoints.last().copied(),
        Some(SLOT + 300),
        "the last record carries the highest checkpoint, which is what a poller stores"
    );
}

#[tokio::test]
async fn no_matching_trade_returns_an_empty_list() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    db.get_ref().markets().insert(test_market()).await.unwrap();

    let records = get_certificates(&address, "").await;
    assert!(records.is_empty());
}

#[tokio::test]
async fn unknown_market_topology_returns_an_empty_list_not_a_partial_record() {
    let app = init_app().await;
    let address = app.address;
    let db = web::Data::new(app.db_wrapper);
    // No market inserted: the seller area hash resolves to nothing.
    db.get_ref()
        .trades()
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, Some(SLOT + 60))])
        .await
        .unwrap();
    db.get_ref()
        .measurements()
        .insert_measurements(vec![production_measurement(SLOT, SLOT + 30)])
        .await
        .unwrap();

    let records = get_certificates(&address, "").await;
    assert!(
        records.is_empty(),
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
        .insert_trades(vec![trade(SLOT, 3.0, TradeStatus::Executed, Some(SLOT + 60))])
        .await
        .unwrap();

    let records = get_certificates(&address, "").await;
    assert!(
        records.is_empty(),
        "seller-side evidence is required: no measurement, no certificate"
    );
}
