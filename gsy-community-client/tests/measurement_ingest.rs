//! The pure parts of the measurement loop: counting the rows per community, describing the
//! metering points, and spotting a partial ontology fetch.

use chrono::{DateTime, Utc};
use gsy_community_client::external_measurements::influxdb_api::InfluxMeasurementMeterData;
use gsy_community_client::external_measurements::manager::{
    CompletenessCounts, IncompleteRow, describe_metering_points, summarize_rows,
};
use gsy_community_client::external_measurements::metering_points::{
    MeterReadings, MeteringPoint, MeteringPointKind, MeteringPointSet, build_metering_points,
    measurement_rows,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    deterministic_area_hash, deterministic_area_uuid, deterministic_community_uuid,
};
use gsy_community_client::topology::{
    LECCommunityAssetsResults, LECCommunityMembersResults, RawOntology,
};
use gsy_offchain_primitives::utils::h256_to_string;
use std::collections::{BTreeMap, BTreeSet, HashMap};

// Live ontology responses, captured on 2026-09-26.
const LECS_BUILDINGS: &str = include_str!("fixtures/lecs_buildings.json");
const ASSETS_PILOT1: &str = include_str!("fixtures/assets_pilot1.json");
const ASSETS_PILOT2: &str = include_str!("fixtures/assets_pilot2.json");
const ASSETS_PILOT3: &str = include_str!("fixtures/assets_pilot3.json");

const SLOT_SEC: u64 = 900;
/// 2026-09-26T00:00:00Z.
const DAY_START: u64 = 1_790_380_800;
const GRACE_SEC: u64 = 165_600;

fn raw(lecs: &[&str]) -> RawOntology {
    let assets = |lec: &str, json: &str| -> (String, LECCommunityAssetsResults) {
        (
            lec.to_string(),
            serde_json::from_str(json).expect("assets fixture parses"),
        )
    };
    let buildings: LECCommunityMembersResults =
        serde_json::from_str(LECS_BUILDINGS).expect("lecs_buildings.json parses");
    RawOntology {
        buildings,
        assets: [
            ("Pilot1", ASSETS_PILOT1),
            ("Pilot2", ASSETS_PILOT2),
            ("Pilot3", ASSETS_PILOT3),
        ]
        .into_iter()
        .filter(|(lec, _)| lecs.contains(lec))
        .map(|(lec, json)| assets(lec, json))
        .collect(),
    }
}

fn point(community: &str, name: &str, kind: MeteringPointKind, meters: &[&str]) -> MeteringPoint {
    MeteringPoint {
        community_name: community.to_string(),
        community_uuid: deterministic_community_uuid(community),
        name: name.to_string(),
        kind,
        area_uuid: deterministic_area_uuid(community, name),
        area_hash: h256_to_string(deterministic_area_hash(community, name)),
        members: BTreeSet::new(),
        member_area_hashes: vec![],
        expected_meters: meters.iter().map(|meter| meter.to_string()).collect(),
    }
}

fn slot(index: u64) -> u64 {
    DAY_START + index * SLOT_SEC
}

fn add_reading(readings: &mut MeterReadings, token: &str, slot: u64) {
    let time = DateTime::<Utc>::from_timestamp(slot as i64, 0).unwrap();
    readings.entry(token.to_string()).or_default().insert(
        time,
        InfluxMeasurementMeterData {
            sensor_id: token.to_string(),
            time,
            import_Wh: Some(100.0),
            export_Wh: Some(0.0),
        },
    );
}

/// Community A: a two-meter house and an unmetered site. Community B: a one-meter house.
/// Community C: a house without readings.
fn test_set() -> MeteringPointSet {
    MeteringPointSet {
        points: vec![
            point("A", "AHouse", MeteringPointKind::Building, &["A01", "A02"]),
            point("A", "ASite", MeteringPointKind::UnmeteredSite, &[]),
            point("B", "BHouse", MeteringPointKind::Building, &["B01"]),
            point("C", "CHouse", MeteringPointKind::Building, &["C01"]),
        ],
        notes: vec![],
        known_meter_tokens: BTreeSet::new(),
    }
}

/// Slot 0: everyone reports. Slot 1: `A02` and `B01` are absent. Slot 2: only `A01`.
fn test_readings() -> MeterReadings {
    let mut readings = MeterReadings::new();
    for token in ["A01", "A02", "B01"] {
        add_reading(&mut readings, token, slot(0));
    }
    add_reading(&mut readings, "A01", slot(1));
    add_reading(&mut readings, "A01", slot(2));
    readings
}

fn counts(complete: usize, incomplete: usize, missing: usize) -> CompletenessCounts {
    CompletenessCounts {
        complete,
        incomplete,
        missing,
    }
}

#[test]
fn summarize_rows_counts_per_community_and_lists_the_incomplete_rows() {
    let set = test_set();
    let slots = [slot(0), slot(1), slot(2)];
    let rows = measurement_rows(
        &set.points,
        &test_readings(),
        &slots,
        slot(2) + GRACE_SEC,
        GRACE_SEC,
    );

    let summary = summarize_rows(&set, &rows);
    assert_eq!(
        summary.counts,
        BTreeMap::from([
            ("A".to_string(), counts(1, 2, 3)),
            ("B".to_string(), counts(1, 0, 2)),
            ("C".to_string(), counts(0, 0, 3)),
        ])
    );
    assert_eq!(
        summary.incomplete,
        vec![
            IncompleteRow {
                community: "A".to_string(),
                point: "AHouse".to_string(),
                time_slot: slot(1),
                missing_meters: vec!["A02".to_string()],
            },
            IncompleteRow {
                community: "A".to_string(),
                point: "AHouse".to_string(),
                time_slot: slot(2),
                missing_meters: vec!["A02".to_string()],
            },
        ]
    );
}

#[test]
fn summarize_rows_before_the_grace_period_counts_only_complete_rows() {
    let set = test_set();
    let rows = measurement_rows(
        &set.points,
        &test_readings(),
        &[slot(0), slot(1), slot(2)],
        slot(3),
        GRACE_SEC,
    );
    let summary = summarize_rows(&set, &rows);
    assert_eq!(summary.counts["A"], counts(1, 0, 0));
    assert_eq!(summary.counts["B"], counts(1, 0, 0));
    // A community without rows is still listed.
    assert_eq!(summary.counts["C"], counts(0, 0, 0));
    assert!(summary.incomplete.is_empty());
}

#[test]
fn summarize_rows_ignores_rows_of_no_point() {
    let set = test_set();
    let mut rows = measurement_rows(
        &set.points,
        &test_readings(),
        &[slot(0)],
        slot(1),
        GRACE_SEC,
    );
    let mut foreign = rows[0].clone();
    foreign.area_hash = h256_to_string(deterministic_area_hash("Z", "ZHouse"));
    rows.push(foreign);
    let mut plain = rows[0].clone();
    plain.metering_point = None;
    rows.push(plain);

    let summary = summarize_rows(&set, &rows);
    assert_eq!(summary.counts.len(), 3);
    assert_eq!(summary.counts["A"], counts(1, 0, 0));
    assert!(summary.counts.values().map(|c| c.complete).sum::<usize>() == 2);

    assert!(
        summarize_rows(&MeteringPointSet::default(), &rows)
            .counts
            .is_empty()
    );
}

#[test]
fn describe_metering_points_gives_one_line_per_site() {
    let raw = raw(&["Pilot1", "Pilot2", "Pilot3"]);
    let set = build_metering_points(&raw.buildings, &raw.assets, &HashMap::new());
    let lines = describe_metering_points(&set);

    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with(
        "ArenaInnovationCommunity: 14 building point(s), 1 unmetered point(s), always \
         missing: ArenaInnovationCommunity (AIC00SGIM, AIC44EV, AIC44SM, AIC49DBATT, AIC49SM, \
         AIC_transformer)"
    ));
    assert_eq!(
        lines[1],
        "GaramèDistrict: 7 building point(s), no unmetered point"
    );
    assert!(
        lines[2]
            .starts_with("LugaggiaInnovationCommunity: 19 building point(s), 1 unmetered point(s)")
    );
    assert!(lines[2].contains("LIC02SM"));
}

#[test]
fn missing_lecs_are_the_lecs_without_assets() {
    assert!(
        raw(&["Pilot1", "Pilot2", "Pilot3"])
            .missing_lecs()
            .is_empty()
    );
    assert_eq!(raw(&["Pilot2"]).missing_lecs(), vec!["Pilot1", "Pilot3"]);
    assert_eq!(raw(&[]).missing_lecs(), vec!["Pilot1", "Pilot2", "Pilot3"]);
}
