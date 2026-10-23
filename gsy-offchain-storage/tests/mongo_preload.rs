//! The MongoDB-only start-up path of `init_database`: duplicate `(area_hash, time_slot)`
//! measurements and duplicate `trade_uuid` trades are removed and the unique indexes backing the
//! measurement upsert and the trade insert are built. Also the Mongo side of the idempotent order
//! insert (unordered `insert_many`, duplicate key errors tolerated).
//!
//! These tests need a running MongoDB and return immediately unless `MONGO_TEST_URL` is set,
//! e.g. `MONGO_TEST_URL=mongodb://127.0.0.1:27917 cargo test -p gsy-offchain-storage --test
//! mongo_preload -- --nocapture`. Each test uses a database of its own and drops it at the end,
//! also when an assertion fails. Set `TEST_LOG` to print the service's log lines.

use futures::{FutureExt, TryStreamExt};
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
};
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use gsy_offchain_storage::db::init_database;
use gsy_offchain_storage::telemetry::{get_subscriber, init_subscriber};
use mongodb::bson::{Bson, Document, doc};
use mongodb::error::{ErrorKind, WriteFailure};
use mongodb::{Client, Collection, IndexModel};
use once_cell::sync::Lazy;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::{SystemTime, UNIX_EPOCH};

const SLOT: i64 = 1_800_000_000;
const DUPLICATE_KEY_ERROR_CODE: i32 = 11000;

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

/// Run `test` with the `MONGO_TEST_URL` and the name of a database no other run uses, then drop
/// that database, also when `test` panics. Does nothing when `MONGO_TEST_URL` is unset.
async fn with_test_database<F, Fut>(name: &str, test: F)
where
    F: FnOnce(String, String) -> Fut,
    Fut: Future<Output = ()>,
{
    let Ok(url) = std::env::var("MONGO_TEST_URL") else {
        eprintln!(
            "MONGO_TEST_URL is not set, skipping the MongoDB test {}",
            name
        );
        return;
    };
    Lazy::force(&TRACING);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let db_name = format!("gsy_{}_{}_{}", name, std::process::id(), nanos);

    let outcome = AssertUnwindSafe(test(url.clone(), db_name.clone()))
        .catch_unwind()
        .await;

    let client = Client::with_uri_str(&url).await.unwrap();
    client.database(&db_name).drop().await.unwrap();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

/// A measurement document as `insert_many` stored it before the upsert existed.
fn raw_measurement(
    area_hash: &str,
    time_slot: i64,
    creation_time: i64,
    energy_kwh: f64,
) -> Document {
    doc! {
        "area_uuid": format!("uuid_{}", area_hash),
        "area_hash": area_hash,
        "community_uuid": "community_1",
        "time_slot": time_slot,
        "creation_time": creation_time,
        "energy_kwh": energy_kwh,
    }
}

/// `(area_hash, time_slot, creation_time)` of every stored measurement, sorted.
async fn stored_keys(measurements: &Collection<Document>) -> Vec<(String, i64, i64)> {
    let rows: Vec<Document> = measurements
        .find(doc! {})
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    let mut keys: Vec<(String, i64, i64)> = rows
        .iter()
        .map(|row| {
            (
                row.get_str("area_hash").unwrap().to_string(),
                row.get_i64("time_slot").unwrap(),
                row.get_i64("creation_time").unwrap(),
            )
        })
        .collect();
    keys.sort();
    keys
}

/// Whether the unique `(area_hash, time_slot)` index backing the measurement upsert exists.
async fn has_unique_key_index(measurements: &Collection<Document>) -> bool {
    let indexes: Vec<IndexModel> = measurements
        .list_indexes()
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    indexes.iter().any(|index| {
        index.keys == doc! {"area_hash": 1, "time_slot": 1}
            && index.options.as_ref().and_then(|options| options.unique) == Some(true)
    })
}

/// A per-area measurement as the service stores it.
fn area_row() -> MeasurementSchema {
    MeasurementSchema {
        area_uuid: "area_1".to_string(),
        area_hash: "0xarea".to_string(),
        community_uuid: "community_1".to_string(),
        time_slot: SLOT as u64,
        creation_time: 1_800_003_600,
        energy_kwh: 1.5,
        metering_point: None,
    }
}

#[tokio::test]
async fn init_database_removes_duplicate_measurements_and_builds_the_unique_index() {
    with_test_database("preload", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let measurements: Collection<Document> =
            client.database(&db_name).collection("measurements");
        // Three rows of one key, stored out of creation_time order, next to rows sharing only
        // the area_hash or only the time_slot with them.
        measurements
            .insert_many(vec![
                raw_measurement("0xdup", SLOT, 1_800_000_200, 2.0),
                raw_measurement("0xdup", SLOT, 1_800_000_300, 3.0),
                raw_measurement("0xdup", SLOT, 1_800_000_100, 1.0),
                raw_measurement("0xdup", SLOT + 900, 1_800_000_100, 4.0),
                raw_measurement("0xother", SLOT, 1_800_000_100, 5.0),
            ])
            .await
            .unwrap();

        init_database(url.clone(), db_name.clone())
            .await
            .expect("start-up must succeed over duplicate measurements");

        let kept = vec![
            ("0xdup".to_string(), SLOT, 1_800_000_300),
            ("0xdup".to_string(), SLOT + 900, 1_800_000_100),
            ("0xother".to_string(), SLOT, 1_800_000_100),
        ];
        assert_eq!(
            stored_keys(&measurements).await,
            kept,
            "one row per (area_hash, time_slot) must remain, the one with the newest creation_time"
        );

        assert!(
            has_unique_key_index(&measurements).await,
            "start-up must build the unique (area_hash, time_slot) index"
        );

        let error = measurements
            .insert_one(raw_measurement("0xdup", SLOT, 1_800_000_400, 9.0))
            .await
            .expect_err("the unique index must reject a second row of a stored key");
        assert!(
            matches!(
                &*error.kind,
                ErrorKind::Write(WriteFailure::WriteError(write_error))
                    if write_error.code == DUPLICATE_KEY_ERROR_CODE
            ),
            "expected a duplicate-key error, got {:?}",
            error
        );

        init_database(url.clone(), db_name.clone())
            .await
            .expect("a second start-up must succeed once the index exists");
        assert_eq!(stored_keys(&measurements).await, kept);
    })
    .await;
}

#[tokio::test]
async fn insert_measurements_upserts_against_mongodb() {
    with_test_database("upsert", |url, db_name| async move {
        let db = init_database(url, db_name).await.unwrap();
        let measurements = db.measurements();
        let area_row = area_row();
        let building_row = MeasurementSchema {
            area_uuid: "building_1".to_string(),
            area_hash: "0xbuilding".to_string(),
            energy_kwh: 0.0,
            metering_point: Some(MeteringPointMeasurement {
                name: "AICHouse11".to_string(),
                member_area_hashes: vec!["0xaaa".to_string(), "0xbbb".to_string()],
                completeness: MeasurementCompleteness::Missing,
                missing_meters: vec!["AIC34".to_string(), "AIC35".to_string()],
            }),
            ..area_row.clone()
        };
        let reposted = |row: &MeasurementSchema| MeasurementSchema {
            creation_time: row.creation_time + 900,
            ..row.clone()
        };
        let completed = MeasurementSchema {
            creation_time: 1_800_090_000,
            energy_kwh: -1.25,
            metering_point: Some(MeteringPointMeasurement {
                completeness: MeasurementCompleteness::Complete,
                missing_meters: vec![],
                ..building_row.metering_point.clone().unwrap()
            }),
            ..building_row.clone()
        };

        assert_eq!(
            measurements
                .insert_measurements(vec![area_row.clone(), building_row.clone()])
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            measurements
                .insert_measurements(vec![reposted(&area_row), reposted(&building_row)])
                .await
                .unwrap(),
            0,
            "rows equal to the stored ones apart from creation_time must not be written"
        );
        assert_eq!(
            measurements
                .insert_measurements(vec![completed.clone()])
                .await
                .unwrap(),
            1
        );

        let mut stored = measurements
            .filter_measurements(None, None, None)
            .await
            .unwrap();
        stored.sort_by(|a, b| a.area_hash.cmp(&b.area_hash));
        assert_eq!(stored, vec![area_row, completed]);
    })
    .await;
}

#[tokio::test]
async fn init_database_continues_when_the_unique_index_cannot_be_built() {
    with_test_database("noindex", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let raw_measurements: Collection<Document> =
            client.database(&db_name).collection("measurements");
        // A non-unique index under the name the unique one would get makes its build fail.
        raw_measurements
            .create_index(
                IndexModel::builder()
                    .keys(doc! {"area_hash": 1, "time_slot": 1})
                    .build(),
            )
            .await
            .unwrap();

        let db = init_database(url, db_name)
            .await
            .expect("a failed unique index build must not stop start-up");
        assert!(
            !has_unique_key_index(&raw_measurements).await,
            "the unique index build was expected to fail"
        );

        // Upserts keep working without the unique index.
        let measurements = db.measurements();
        assert_eq!(
            measurements
                .insert_measurements(vec![area_row()])
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            measurements
                .insert_measurements(vec![area_row()])
                .await
                .unwrap(),
            0
        );
    })
    .await;
}

/// A trade record as the service stores it, before any status change, with `_id`
/// `<trade_uuid>-<id>` (an `_id` is unique across the collection).
fn trade(id: &str, trade_uuid: &str) -> TradeSchema {
    let id = &format!("{}-{}", trade_uuid, id);
    let component = |area_uuid: &str| DbOrderComponent {
        area_uuid: area_uuid.to_string(),
        market_id: "market".to_string(),
        time_slot: SLOT as u64,
        creation_time: 1_799_990_000,
        energy: 2.0,
        energy_rate: 0.1,
    };
    TradeSchema {
        _id: id.to_string(),
        status: TradeStatus::Settled,
        seller: "seller".to_string(),
        buyer: "buyer".to_string(),
        market_id: "market".to_string(),
        time_slot: SLOT as u64,
        trade_uuid: trade_uuid.to_string(),
        creation_time: 1_799_990_000,
        status_updated_at: None,
        offer: DbOffer {
            seller: "seller".to_string(),
            nonce: 1,
            offer_component: component("0xseller"),
        },
        offer_hash: format!("offer_{}", id),
        bid: DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: component("0xbuyer"),
        },
        bid_hash: format!("bid_{}", id),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy: 2.0,
            energy_rate: 0.1,
            trade_uuid: trade_uuid.to_string(),
        },
    }
}

fn judged(mut trade: TradeSchema, status: TradeStatus, status_updated_at: u64) -> TradeSchema {
    trade.status = status;
    trade.status_updated_at = Some(status_updated_at);
    trade
}

/// Every stored trade, sorted by `(trade_uuid, _id)`.
async fn stored_trades(trades: &Collection<TradeSchema>) -> Vec<TradeSchema> {
    let mut rows: Vec<TradeSchema> = trades
        .find(doc! {})
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    rows.sort_by(|a, b| (&a.trade_uuid, &a._id).cmp(&(&b.trade_uuid, &b._id)));
    rows
}

#[tokio::test]
async fn init_database_removes_duplicate_trades_and_builds_the_unique_index() {
    with_test_database("trades", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let raw_trades: Collection<TradeSchema> = client.database(&db_name).collection("trades");
        let executed = judged(trade("c", "judged"), TradeStatus::Executed, 1_800_010_000);
        let first_verdict = judged(
            trade("z", "two_verdicts"),
            TradeStatus::Penalized,
            1_800_010_000,
        );
        let mut oldest = trade("y", "settled");
        oldest.creation_time -= 60;
        let smallest_id = trade("a", "same_age");
        let single = trade("s", "single");
        // Copies of one trade as the old insert stored every post, next to a trade stored once.
        raw_trades
            .insert_many(vec![
                trade("a", "judged"),
                executed.clone(),
                trade("b", "judged"),
                judged(
                    trade("a", "two_verdicts"),
                    TradeStatus::Executed,
                    1_800_020_000,
                ),
                first_verdict.clone(),
                trade("b", "settled"),
                oldest.clone(),
                trade("b", "same_age"),
                smallest_id.clone(),
                single.clone(),
            ])
            .await
            .unwrap();

        let db = init_database(url.clone(), db_name.clone())
            .await
            .expect("start-up must succeed over duplicate trades");

        let kept = vec![executed, smallest_id, oldest, single, first_verdict];
        assert_eq!(
            stored_trades(&raw_trades).await,
            kept,
            "one record per trade_uuid must remain: a judged one, else the oldest"
        );

        let indexes: Vec<IndexModel> = raw_trades
            .list_indexes()
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();
        assert!(
            indexes
                .iter()
                .any(|index| index.keys == doc! {"trade_uuid": 1}
                    && index.options.as_ref().and_then(|options| options.unique) == Some(true)),
            "start-up must build the unique trade_uuid index"
        );
        let error = raw_trades
            .insert_one(trade("d", "single"))
            .await
            .expect_err("the unique index must reject a second record of a stored trade");
        assert!(
            matches!(
                &*error.kind,
                ErrorKind::Write(WriteFailure::WriteError(write_error))
                    if write_error.code == DUPLICATE_KEY_ERROR_CODE
            ),
            "expected a duplicate-key error, got {:?}",
            error
        );

        // A re-post through the service succeeds, stores nothing and keeps the verdict.
        let inserted = db
            .trades()
            .insert_trades(vec![trade("e", "judged"), trade("f", "new")])
            .await
            .expect("a re-post must succeed");
        assert_eq!(inserted.len(), 1);
        assert!(inserted.contains_key(&1));
        let stored = stored_trades(&raw_trades).await;
        assert_eq!(stored.len(), kept.len() + 1);
        assert!(
            stored.contains(&kept[0]),
            "the judged record must be left untouched"
        );

        init_database(url.clone(), db_name.clone())
            .await
            .expect("a second start-up must succeed once the index exists");
        assert_eq!(stored_trades(&raw_trades).await, stored);
    })
    .await;
}

#[tokio::test]
async fn trade_status_updates_reach_every_copy_against_mongodb() {
    with_test_database("tradestatus", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let raw_trades: Collection<TradeSchema> = client.database(&db_name).collection("trades");
        let db = init_database(url, db_name).await.unwrap();
        // Two copies of one trade, as if the unique index could not be built.
        raw_trades
            .drop_index("trade_uuid_1")
            .await
            .expect("start-up builds the unique trade_uuid index");
        raw_trades
            .insert_many(vec![
                trade("a", "dup"),
                trade("b", "dup"),
                trade("c", "other"),
            ])
            .await
            .unwrap();

        let summary = db
            .trades()
            .update_trade_status_by_uuid("dup", TradeStatus::Executed)
            .await
            .unwrap();
        assert_eq!(summary.matched_count, 2);
        assert_eq!(summary.modified_count, 2);
        let statuses: Vec<(String, TradeStatus)> = stored_trades(&raw_trades)
            .await
            .into_iter()
            .map(|trade| (trade._id, trade.status))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("dup-a".to_string(), TradeStatus::Executed),
                ("dup-b".to_string(), TradeStatus::Executed),
                ("other-c".to_string(), TradeStatus::Settled),
            ]
        );
    })
    .await;
}

/// An `Open` bid as the service stores it, with `_id` `id`.
fn order(id: &str) -> DbOrderSchema {
    DbOrderSchema {
        _id: id.to_string(),
        status: OrderStatus::Open,
        order: Order::Bid(DbBid {
            buyer: "buyer".to_string(),
            nonce: 1,
            bid_component: DbOrderComponent {
                area_uuid: "area".to_string(),
                market_id: "market".to_string(),
                time_slot: SLOT as u64,
                creation_time: 1_799_990_000,
                energy: 1.0,
                energy_rate: 0.3,
            },
        }),
    }
}

/// `(_id, status)` of every stored order, sorted by `_id`.
async fn stored_order_statuses(orders: &Collection<DbOrderSchema>) -> Vec<(String, OrderStatus)> {
    let mut rows: Vec<(String, OrderStatus)> = orders
        .find(doc! {})
        .await
        .unwrap()
        .try_collect::<Vec<DbOrderSchema>>()
        .await
        .unwrap()
        .into_iter()
        .map(|order| (order._id, order.status))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

/// The inserted `_id`s reported by `insert_orders`, as `(index, _id)` sorted by index.
fn reported(ids: std::collections::HashMap<usize, Bson>) -> Vec<(usize, String)> {
    let mut ids: Vec<(usize, String)> = ids
        .into_iter()
        .map(|(index, id)| (index, id.as_str().unwrap().to_string()))
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn insert_orders_skips_stored_orders_against_mongodb() {
    with_test_database("orderrepost", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let raw_orders: Collection<DbOrderSchema> = client.database(&db_name).collection("orders");
        let db = init_database(url, db_name).await.unwrap();
        let orders = db.orders();

        let inserted = orders
            .insert_orders(vec![order("a"), order("b")])
            .await
            .unwrap();
        assert_eq!(
            reported(inserted),
            vec![(0, "a".to_string()), (1, "b".to_string())]
        );
        orders
            .update_order_status_by_id(&Bson::String("a".to_string()), OrderStatus::Executed)
            .await
            .unwrap();

        // A re-post of the whole batch succeeds, stores nothing and keeps the status.
        let inserted = orders
            .insert_orders(vec![order("a"), order("b")])
            .await
            .unwrap();
        assert!(
            inserted.is_empty(),
            "nothing new was stored: {:?}",
            inserted
        );

        // The stored orders first: the unordered insert still stores the rest, and a duplicate
        // within the batch is stored once.
        let inserted = orders
            .insert_orders(vec![
                order("a"),
                order("c"),
                order("b"),
                order("d"),
                order("c"),
            ])
            .await
            .unwrap();
        assert_eq!(
            reported(inserted),
            vec![(1, "c".to_string()), (3, "d".to_string())]
        );

        assert_eq!(
            stored_order_statuses(&raw_orders).await,
            vec![
                ("a".to_string(), OrderStatus::Executed),
                ("b".to_string(), OrderStatus::Open),
                ("c".to_string(), OrderStatus::Open),
                ("d".to_string(), OrderStatus::Open),
            ]
        );
    })
    .await;
}

#[tokio::test]
async fn insert_orders_fails_on_other_write_errors_against_mongodb() {
    with_test_database("orderwriteerror", |url, db_name| async move {
        let client = Client::with_uri_str(&url).await.unwrap();
        let database = client.database(&db_name);
        // A validator that rejects `Deleted` orders stands in for any write error other than a
        // duplicate key.
        database
            .create_collection("orders")
            .validator(doc! {"status": {"$ne": "Deleted"}})
            .await
            .unwrap();
        let raw_orders: Collection<DbOrderSchema> = database.collection("orders");
        let db = init_database(url, db_name).await.unwrap();
        let orders = db.orders();
        orders.insert_orders(vec![order("a")]).await.unwrap();

        let mut rejected = order("b");
        rejected.status = OrderStatus::Deleted;
        let result = orders
            .insert_orders(vec![order("a"), rejected, order("c")])
            .await;

        assert!(
            result.is_err(),
            "a non-duplicate write error must fail the insert"
        );
        // The insert is unordered, so the valid new order is stored all the same.
        assert_eq!(
            stored_order_statuses(&raw_orders).await,
            vec![
                ("a".to_string(), OrderStatus::Open),
                ("c".to_string(), OrderStatus::Open),
            ]
        );
    })
    .await;
}
