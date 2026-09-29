use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::db::DatabaseWrapper;
use primitives::certificates::{DeliveryScope, LocalOriginRecord};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema,
};
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus};
use primitives::utils::{
    bytes16_to_hex, create_encrypted_bytes16_from_string, timestamp_to_string_with_padding,
};

pub const SLOT: u64 = 1_778_754_600; // 2026-05-14T10:30:00Z
/// When the seeded trades reached `Executed` (their `status_updated_at`).
pub const VERDICT_AT: u64 = SLOT + 1_800;
pub const SELLER_FACILITY: &str = "goo-seller";
const SELLER_OWNER: &str = "goo-owner-seller";
const BUYER_FACILITY: &str = "goo-buyer";
const BUYER_OWNER: &str = "goo-owner-buyer";
const COMMUNITY: &str = "goo-community";
const EXPORT_POINT: &str = "Measurement:goo-community:goo-seller";

fn hash(offchain_id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(offchain_id))
}

/// A 2 kWh sale from the seller facility to the buyer facility at `SLOT`.
pub fn trade(
    trade_uuid: &str,
    status: TradeStatus,
    status_updated_at: Option<u64>,
) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: trade_uuid.to_string(),
        status,
        seller: hash(SELLER_OWNER),
        buyer: hash(BUYER_OWNER),
        market_id: "0xmarket".to_string(),
        time_slot: SLOT,
        creation_time: SLOT - 600,
        offer_hash: format!("{trade_uuid}-offer"),
        bid_hash: format!("{trade_uuid}-bid"),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 2.0,
            energy_rate: 0.2,
        },
        status_updated_at,
    }
}

/// Seeds a PV seller facility exporting `export_kwh` at `SLOT` and a buyer facility in the
/// same community. No trades.
pub async fn seed_topology(db: &DatabaseWrapper, export_kwh: f64) {
    for (facility_id, owner_id) in [
        (SELLER_FACILITY, SELLER_OWNER),
        (BUYER_FACILITY, BUYER_OWNER),
    ] {
        db.facilities()
            .insert(FacilitySchema {
                facility_id: facility_id.to_string(),
                facility_name: format!("{facility_id}-name"),
                site_id: "goo-site".to_string(),
                owner_id: owner_id.to_string(),
            })
            .await
            .unwrap();
    }
    db.communities()
        .insert(EnergyCommunitySchema {
            community_id: COMMUNITY.to_string(),
            community_name: "GoO Community".to_string(),
            sites: vec!["goo-site".to_string()],
        })
        .await
        .unwrap();
    db.assets()
        .insert_assets(vec![AssetSchema {
            asset_type: AssetType::PV,
            uuid: "goo-pv".to_string(),
            asset_name: "goo-pv".to_string(),
            facility_name: format!("{SELLER_FACILITY}-name"),
            creation_time: 1,
            installed_power: Some(10.0),
            asset_subtype: None,
            technology_type: None,
            phase_connection: None,
            energy_capacity: None,
            maximum_soc: None,
            minimum_soc: None,
            roundtrip_efficiency: None,
            target_service: None,
            grid_connection_type: None,
            max_rated_current: None,
            has_smart_meter: None,
            tariff_name: None,
        }])
        .await
        .unwrap();
    db.measurement_points()
        .insert_points(vec![MeasurementPointSchema {
            point_type: MeasurementPointType::Measurement,
            measurement_id: EXPORT_POINT.to_string(),
            property_measured: "energy_measured".to_string(),
            unit: "kWh".to_string(),
            direction: FlowDirection::Export,
            energy_accumulated: false,
            time_resolution: "PT15M".to_string(),
            phase: 0,
            asset_name: SELLER_FACILITY.to_string(),
            datasource_name: Some(COMMUNITY.to_string()),
        }])
        .await
        .unwrap();
    db.timeseries()
        .insert_values(vec![TimeseriesSchema {
            measurement_point: EXPORT_POINT.to_string(),
            timestamp: timestamp_to_string_with_padding(SLOT),
            value: export_kwh,
        }])
        .await
        .unwrap();
}

/// [`seed_topology`] with a 3 kWh export, plus one 2 kWh sale `trade_uuid` inserted directly
/// as `Executed` with `status_updated_at = VERDICT_AT`.
pub async fn seed_certifiable_trade(db: &DatabaseWrapper, trade_uuid: &str) {
    seed_topology(db, 3.0).await;
    db.trades()
        .insert_trades(vec![trade(
            trade_uuid,
            TradeStatus::Executed,
            Some(VERDICT_AT),
        )])
        .await
        .unwrap();
}

fn sorted_uuids(trades: Vec<DbTradeSchema>) -> Vec<String> {
    let mut uuids: Vec<String> = trades.into_iter().map(|t| t.trade_uuid).collect();
    uuids.sort();
    uuids
}

// --- filter_trades_by_status_change ---------------------------------

#[tokio::test]
async fn filter_trades_by_status_change_bounds_are_inclusive() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    db.trades()
        .insert_trades(vec![
            trade("t-99", TradeStatus::Executed, Some(99)),
            trade("t-100", TradeStatus::Executed, Some(100)),
            trade("t-150", TradeStatus::Settled, Some(150)),
            trade("t-200", TradeStatus::Executed, Some(200)),
            trade("t-201", TradeStatus::Executed, Some(201)),
        ])
        .await
        .unwrap();

    // [100, 200], both inclusive, any status.
    let all = db
        .trades()
        .filter_trades_by_status_change(100, Some(200), None)
        .await
        .unwrap();
    assert_eq!(sorted_uuids(all), vec!["t-100", "t-150", "t-200"]);

    // Restricted to Executed.
    let executed = db
        .trades()
        .filter_trades_by_status_change(100, Some(200), Some(TradeStatus::Executed))
        .await
        .unwrap();
    assert_eq!(sorted_uuids(executed), vec!["t-100", "t-200"]);

    // A single-instant window.
    let instant = db
        .trades()
        .filter_trades_by_status_change(200, Some(200), None)
        .await
        .unwrap();
    assert_eq!(sorted_uuids(instant), vec!["t-200"]);

    // Open-ended upper bound.
    let open = db
        .trades()
        .filter_trades_by_status_change(150, None, None)
        .await
        .unwrap();
    assert_eq!(sorted_uuids(open), vec!["t-150", "t-200", "t-201"]);

    stop_app(app).await;
}

#[tokio::test]
async fn filter_trades_by_status_change_excludes_trades_without_a_verdict() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    db.trades()
        .insert_trades(vec![
            trade("t-verdict", TradeStatus::Executed, Some(100)),
            trade("t-none-executed", TradeStatus::Executed, None),
            trade("t-none-settled", TradeStatus::Settled, None),
        ])
        .await
        .unwrap();

    // Even the widest window never matches a trade without `status_updated_at`.
    let all = db
        .trades()
        .filter_trades_by_status_change(0, None, None)
        .await
        .unwrap();
    assert_eq!(sorted_uuids(all), vec!["t-verdict"]);

    let executed = db
        .trades()
        .filter_trades_by_status_change(0, Some(u64::MAX >> 1), Some(TradeStatus::Executed))
        .await
        .unwrap();
    assert_eq!(sorted_uuids(executed), vec!["t-verdict"]);

    stop_app(app).await;
}

// --- REST: GET /guarantees-of-origin-measurements --------------------

async fn get_goo(address: &str, query: &[(&str, String)]) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!("{}/guarantees-of-origin-measurements", address))
        .query(query)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn rest_rejects_an_invalid_window_with_400() {
    let app = init_app().await;

    for query in [
        vec![],
        vec![("end_time", "1000".to_string())],
        vec![
            ("start_time", "1000".to_string()),
            ("end_time", "999".to_string()),
        ],
        vec![
            ("start_time", "1000".to_string()),
            ("end_time", "1901".to_string()),
        ],
    ] {
        let response = get_goo(&app.address, &query).await;
        assert_eq!(400, response.status().as_u16(), "query {:?}", query);
    }

    stop_app(app).await;
}

#[tokio::test]
async fn rest_returns_an_empty_list_when_nothing_was_executed() {
    let app = init_app().await;

    let response = get_goo(&app.address, &[("start_time", "1000".to_string())]).await;
    assert_eq!(200, response.status().as_u16());
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert!(records.is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn rest_returns_a_record_for_a_trade_executed_in_the_window() {
    let app = init_app().await;
    seed_certifiable_trade(&app.db_wrapper, "goo-trade-1").await;

    let response = get_goo(
        &app.address,
        &[
            ("start_time", (VERDICT_AT - 450).to_string()),
            ("end_time", (VERDICT_AT + 450).to_string()),
        ],
    )
    .await;
    assert_eq!(200, response.status().as_u16());
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(
        record.trade_and_delivery.trade_reference,
        vec!["goo-trade-1".to_string()]
    );
    assert_eq!(record.identity.site_id, "goo-site");
    assert_eq!(record.production_asset.production_asset_id, SELLER_FACILITY);
    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        BUYER_FACILITY
    );
    assert_eq!(
        record.location.delivery_scope,
        DeliveryScope::IntraCommunity
    );
    assert_eq!(record.measurement_provenance.measurement_id, EXPORT_POINT);
    assert_eq!(record.time_and_quantity.energy_quantity, 2.0);
    assert_eq!(record.time_and_quantity.source_slot_timestamp, SLOT);

    // The window is on verdict time, not delivery time: a window over the delivery slot
    // itself selects nothing.
    let response = get_goo(&app.address, &[("start_time", SLOT.to_string())]).await;
    assert_eq!(200, response.status().as_u16());
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert!(records.is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn rest_ignores_trades_that_are_not_executed_or_have_no_verdict_time() {
    let app = init_app().await;
    seed_topology(&app.db_wrapper, 10.0).await;
    app.db_wrapper
        .trades()
        .insert_trades(vec![
            trade("goo-settled", TradeStatus::Settled, Some(VERDICT_AT)),
            trade("goo-no-verdict", TradeStatus::Executed, None),
        ])
        .await
        .unwrap();

    let response = get_goo(
        &app.address,
        &[("start_time", (VERDICT_AT - 450).to_string())],
    )
    .await;
    assert_eq!(200, response.status().as_u16());
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert!(records.is_empty());

    stop_app(app).await;
}

#[tokio::test]
async fn rest_allocation_counts_executed_sales_outside_the_window() {
    let app = init_app().await;
    // 3 kWh net export; two 2 kWh sales of the same slot.
    seed_topology(&app.db_wrapper, 3.0).await;
    let mut earlier = trade(
        "goo-earlier",
        TradeStatus::Executed,
        Some(VERDICT_AT - 3_600),
    );
    earlier.creation_time = SLOT - 900;
    let later = trade("goo-later", TradeStatus::Executed, Some(VERDICT_AT));
    app.db_wrapper
        .trades()
        .insert_trades(vec![earlier, later])
        .await
        .unwrap();

    // Only the later sale is in the window, but the earlier one (verdict outside the
    // window) took 2 of the 3 kWh first, so the later sale is not fully covered.
    let response = get_goo(
        &app.address,
        &[("start_time", (VERDICT_AT - 450).to_string())],
    )
    .await;
    assert_eq!(200, response.status().as_u16());
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert!(records.is_empty());

    // The earlier sale's own window certifies it.
    let response = get_goo(
        &app.address,
        &[("start_time", (VERDICT_AT - 3_600).to_string())],
    )
    .await;
    let records: Vec<LocalOriginRecord> = response.json().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].trade_and_delivery.trade_reference,
        vec!["goo-earlier".to_string()]
    );

    stop_app(app).await;
}
