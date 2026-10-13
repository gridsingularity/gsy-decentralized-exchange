//! Live dry run of the measurement loop against the real ontology and InfluxDB. It returns
//! at once unless `LIVE_METERING_DRY_RUN=1`, and needs `FEDECOM_INFLUX_DB_TOKEN`. It only
//! reads: nothing is ever posted to storage.

use chrono::{DateTime, Utc};
use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_measurements::influxdb_api::MeasurementInfluxDBConnection;
use gsy_community_client::external_measurements::manager::{
    describe_metering_points, summarize_rows,
};
use gsy_community_client::external_measurements::metering_points::{
    build_metering_points, ended_slots, measurement_rows, parse_overrides, unmapped_tokens,
};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use gsy_community_client::topology::TopologyManager;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementCompleteness;
use reqwest::Client;

#[tokio::test]
async fn live_metering_dry_run() {
    if std::env::var("LIVE_METERING_DRY_RUN").as_deref() != Ok("1") {
        return;
    }

    let manager = TopologyManager::new(&Client::new(), &AreaMarketInfoAdapter::new(None));
    let raw = manager
        .fetch_raw_ontology()
        .await
        .expect("the ontology answers");
    assert!(
        raw.missing_lecs().is_empty(),
        "missing LECs: {:?}",
        raw.missing_lecs()
    );
    let overrides = parse_overrides(&CommunityClientConstants.METERING_POINT_OVERRIDES)
        .expect("METERING_POINT_OVERRIDES parses");
    let set = build_metering_points(&raw.buildings, &raw.assets, &overrides);
    for note in &set.notes {
        println!("note: {note}");
    }
    for line in describe_metering_points(&set) {
        println!("points: {line}");
    }
    assert_eq!(set.points.len(), 42);

    let now = get_current_timestamp_in_secs();
    let lookback_sec = CommunityClientConstants.MEASUREMENT_LOOKBACK_SEC;
    let to_time = |secs: u64| DateTime::<Utc>::from_timestamp(secs as i64, 0).unwrap();
    let started = std::time::Instant::now();
    let readings = MeasurementInfluxDBConnection::new()
        .read(to_time(now - lookback_sec), to_time(now))
        .await;
    println!(
        "influx: {} meter(s) with readings over the last {lookback_sec}s, read in {:.1}s",
        readings.len(),
        started.elapsed().as_secs_f64()
    );
    println!("unmapped tokens: {:?}", unmapped_tokens(&set, &readings));

    let slots = ended_slots(now, lookback_sec, GlobalConstants.TIME_SLOT_SEC);
    let rows = measurement_rows(
        &set.points,
        &readings,
        &slots,
        now,
        CommunityClientConstants.MEASUREMENT_MISSING_AFTER_SEC,
    );
    let summary = summarize_rows(&set, &rows);
    println!("rows: {} over {} slot(s)", rows.len(), slots.len());
    for (community, counts) in &summary.counts {
        println!(
            "rows of {community}: {} complete, {} incomplete, {} missing",
            counts.complete, counts.incomplete, counts.missing
        );
    }
    for row in &summary.incomplete {
        println!(
            "incomplete: {} ({}) slot {} missing {:?}",
            row.point, row.community, row.time_slot, row.missing_meters
        );
    }
    assert!(rows.iter().any(|row| {
        row.metering_point.as_ref().unwrap().completeness == MeasurementCompleteness::Complete
    }));
}
