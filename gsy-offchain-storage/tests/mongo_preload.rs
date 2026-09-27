//! The MongoDB-only start-up path of `init_database`: duplicate `(area_hash, time_slot)`
//! measurements are removed and the unique index backing the measurement upsert is built.
//!
//! These tests need a running MongoDB and return immediately unless `MONGO_TEST_URL` is set,
//! e.g. `MONGO_TEST_URL=mongodb://127.0.0.1:27917 cargo test -p gsy-offchain-storage --test
//! mongo_preload -- --nocapture`. Each test uses a database of its own and drops it at the end,
//! also when an assertion fails. Set `TEST_LOG` to print the service's log lines.

use futures::{FutureExt, TryStreamExt};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
};
use gsy_offchain_storage::db::init_database;
use gsy_offchain_storage::telemetry::{get_subscriber, init_subscriber};
use mongodb::bson::{Document, doc};
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
