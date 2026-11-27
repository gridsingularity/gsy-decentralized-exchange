use crate::evm_integration::expected_market;
use crate::guarantees_of_origin::trade;
use crate::helpers::{forget_migrations, init_app, restart, stop_app, ts};
use chrono::{DateTime, FixedOffset, Utc};
use futures::TryStreamExt;
use mongodb::Collection;
use mongodb::bson::{self, Bson, Document, doc};
use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema};
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::db_api_schema::orders::{DbOrderSchema, OrderEnum, OrderStatus};
use primitives::db_api_schema::profiles::TimeseriesSchema;
use primitives::db_api_schema::trades::{ClearingResultSchema, ClearingStatus, TradeStatus};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

/// Epoch seconds, as orders, trades, clearing results and ID mappings stored them.
fn epoch(time: DateTime<Utc>) -> Bson {
    Bson::Int64(time.timestamp())
}

/// Zero-padded epoch seconds, as markets and timeseries values stored them.
fn padded(time: DateTime<Utc>) -> Bson {
    Bson::String(format!("{:020}", time.timestamp()))
}

/// RFC 3339 with an offset other than UTC; topology stored UTC, but any offset parses.
fn rfc3339(time: DateTime<Utc>) -> Bson {
    Bson::String(
        time.with_timezone(&FixedOffset::east_opt(3600).unwrap())
            .to_rfc3339(),
    )
}

/// Stores `value` in `collection` the way it was stored before times were dates: `legacy`
/// replaces its time fields.
async fn insert_legacy<T: Serialize + Send + Sync>(
    collection: &Collection<T>,
    value: &T,
    legacy: Document,
) {
    let mut document = bson::to_document(value).unwrap();
    document.extend(legacy);
    collection
        .clone_with_type::<Document>()
        .insert_one(document)
        .await
        .unwrap();
}

/// The documents of `collection` as `T`; fails if one of them is not readable.
async fn read_all<T: DeserializeOwned + Send + Sync>(collection: &Collection<T>) -> Vec<T> {
    collection
        .find(doc! {})
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap()
}

async fn assert_stored<T>(collection: &Collection<T>, expected: Vec<T>)
where
    T: DeserializeOwned + Send + Sync + PartialEq + Debug,
{
    assert_eq!(read_all(collection).await, expected);
}

#[tokio::test]
async fn startup_converts_legacy_times_to_dates() {
    let app = init_app().await;
    let db = &app.db_wrapper;

    let market = expected_market();
    insert_legacy(
        &db.markets(),
        &market,
        doc! {
            "opening_time": padded(market.opening_time),
            "closing_time": padded(market.closing_time),
            "delivery_start_time": padded(market.delivery_start_time),
            "delivery_end_time": padded(market.delivery_end_time),
            "created_at": padded(market.created_at),
        },
    )
    .await;

    let placed = DbOrderSchema {
        order_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd".to_string(),
        status: OrderStatus::Submitted,
        status_updated_at: Some(ts(1_699_998_500)),
        order_type: OrderEnum::Bid,
        market_id: market.market_id.clone(),
        time_slot: market.delivery_start_time,
        creation_time: ts(1_699_998_400),
        energy_kWh: 1.0,
        energy_rate: 0.2,
        created_by: "owner-1".to_string(),
        requirements: None,
        attributes: None,
    };
    insert_legacy(
        &db.orders(),
        &placed,
        doc! {
            "status_updated_at": epoch(ts(1_699_998_500)),
            "time_slot": epoch(placed.time_slot),
            // Small values were stored as 32-bit integers.
            "creation_time": Bson::Int32(1_699_998_400),
        },
    )
    .await;
    let imported = DbOrderSchema {
        order_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee".to_string(),
        status_updated_at: None,
        ..placed.clone()
    };
    insert_legacy(
        &db.orders(),
        &imported,
        doc! {
            "time_slot": epoch(imported.time_slot),
            "creation_time": epoch(imported.creation_time),
        },
    )
    .await;

    let executed = trade("legacy-trade", TradeStatus::Executed, Some(1_700_002_000));
    insert_legacy(
        &db.trades(),
        &executed,
        doc! {
            "creation_time": epoch(executed.creation_time),
            "status_updated_at": epoch(ts(1_700_002_000)),
        },
    )
    .await;

    let clearing = ClearingResultSchema {
        market_id: market.market_id.clone(),
        clearing_status: ClearingStatus::Final,
        no_bid_reason: None,
        clearing_price: 0.2,
        total_supply: 1.0,
        total_demand: 1.0,
        traded_quantity: 1.0,
        num_trades: 1,
        tx_hash: "0x01".to_string(),
        clearing_time: ts(1_700_000_200),
    };
    insert_legacy(
        &db.clearing_results(),
        &clearing,
        doc! {"clearing_time": epoch(clearing.clearing_time)},
    )
    .await;

    let value = TimeseriesSchema {
        measurement_point: "Measurement:community-1:facility-1".to_string(),
        timestamp: market.delivery_start_time,
        value: 1.5,
    };
    insert_legacy(
        &db.timeseries(),
        &value,
        doc! {"timestamp": padded(value.timestamp)},
    )
    .await;

    let mapping = IdMappingSchema {
        offchain_id: "owner-1".to_string(),
        onchain_id: "0x00112233445566778899aabbccddeeff".to_string(),
        creation_time: ts(1_699_990_000),
    };
    insert_legacy(
        &db.ids(),
        &mapping,
        doc! {"creation_time": epoch(mapping.creation_time)},
    )
    .await;

    let community = EnergyCommunitySchema {
        community_id: "community-1".to_string(),
        community_name: "Community 1".to_string(),
        sites: vec!["site-1".to_string()],
        valid_from: ts(0),
        valid_until: None,
    };
    insert_legacy(
        &db.communities(),
        &community,
        doc! {"valid_from": "1970-01-01T00:00:00+00:00"},
    )
    .await;
    let facility = FacilitySchema {
        facility_id: "facility-1".to_string(),
        facility_name: "Facility 1".to_string(),
        site_id: "site-1".to_string(),
        owner_id: "owner-1".to_string(),
        valid_from: ts(1_764_547_200),
        valid_until: Some("2026-12-02T00:00:00.123Z".parse().unwrap()),
    };
    insert_legacy(
        &db.facilities(),
        &facility,
        doc! {
            "valid_from": rfc3339(facility.valid_from),
            // Sub-millisecond digits are dropped, like for any time parsed from RFC 3339.
            "valid_until": "2026-12-02T00:00:00.123456Z",
        },
    )
    .await;

    forget_migrations(&app).await;
    restart(&app).await;
    // Repeated, as after a crash before it was recorded, it changes nothing.
    forget_migrations(&app).await;
    restart(&app).await;

    assert_stored(&db.markets(), vec![market]).await;
    assert_stored(&db.orders(), vec![placed, imported]).await;
    assert_stored(&db.trades(), vec![executed]).await;
    assert_stored(&db.clearing_results(), vec![clearing]).await;
    assert_stored(&db.timeseries(), vec![value]).await;
    assert_stored(&db.ids(), vec![mapping]).await;
    assert_stored(&db.communities(), vec![community]).await;
    assert_stored(&db.facilities(), vec![facility]).await;

    stop_app(app).await;
}

#[tokio::test]
async fn startup_leaves_a_value_that_is_not_a_time_unchanged() {
    let app = init_app().await;
    let timeseries = app.db_wrapper.timeseries().clone_with_type::<Document>();
    timeseries
        .insert_one(doc! {"measurement_point": "point-1", "timestamp": "yesterday", "value": 1.0})
        .await
        .unwrap();

    forget_migrations(&app).await;
    restart(&app).await;

    let stored = timeseries.find_one(doc! {}).await.unwrap().unwrap();
    assert_eq!(stored.get_str("timestamp").unwrap(), "yesterday");

    stop_app(app).await;
}

#[tokio::test]
async fn a_recorded_migration_does_not_run_again() {
    // The start on the new database recorded every migration.
    let app = init_app().await;
    let applied = app.db_wrapper.collection::<Document>("migrations");
    let mut names = applied
        .distinct("_id", doc! {})
        .await
        .unwrap()
        .into_iter()
        .map(|name| name.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["bson_dates", "topology_versioning"]);

    let timeseries = app.db_wrapper.timeseries().clone_with_type::<Document>();
    timeseries
        .insert_one(doc! {"measurement_point": "point-1", "timestamp": "00000000001700000000", "value": 1.0})
        .await
        .unwrap();

    restart(&app).await;

    let stored = timeseries.find_one(doc! {}).await.unwrap().unwrap();
    assert_eq!(stored.get_str("timestamp").unwrap(), "00000000001700000000");

    stop_app(app).await;
}
