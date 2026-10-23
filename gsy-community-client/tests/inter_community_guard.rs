//! The inter-community eligible list and the demand guard: a community joins the
//! inter-community market for a slot only with a demand forecast for every expected
//! SmartMeter of its buildings.

use gsy_community_client::constants::DEFAULT_INTER_COMMUNITY_ELIGIBLE_COMMUNITIES;
use gsy_community_client::external_measurements::metering_points::{
    MeteringPoint, MeteringPointKind, MeteringPointSet, build_metering_points,
};
use gsy_community_client::inter_community::{
    DemandCoverage, demand_coverage, eligible_inter_communities, eligible_inter_community,
    parse_community_list,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    deterministic_area_hash, deterministic_area_uuid, deterministic_community_uuid,
};
use gsy_community_client::topology::{LECCommunityAssetsResults, LECCommunityMembersResults};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::h256_to_string;
use std::collections::{BTreeSet, HashMap};

// Live ontology responses, captured on 2026-09-26.
const LECS_BUILDINGS: &str = include_str!("fixtures/lecs_buildings.json");
const ASSETS_PILOT1: &str = include_str!("fixtures/assets_pilot1.json");
const ASSETS_PILOT2: &str = include_str!("fixtures/assets_pilot2.json");
const ASSETS_PILOT3: &str = include_str!("fixtures/assets_pilot3.json");

const LUGAGGIA: &str = "LugaggiaInnovationCommunity";
const GARAME: &str = "GaramèDistrict";
const ARENA: &str = "ArenaInnovationCommunity";
const SLOT: u64 = 1_790_380_800;

fn live_set() -> MeteringPointSet {
    let buildings: LECCommunityMembersResults =
        serde_json::from_str(LECS_BUILDINGS).expect("lecs_buildings.json parses");
    let assets = |lec: &str, json: &str| -> (String, LECCommunityAssetsResults) {
        (
            lec.to_string(),
            serde_json::from_str(json).expect("assets fixture parses"),
        )
    };
    build_metering_points(
        &buildings,
        &[
            assets("Pilot1", ASSETS_PILOT1),
            assets("Pilot2", ASSETS_PILOT2),
            assets("Pilot3", ASSETS_PILOT3),
        ],
        &HashMap::new(),
    )
}

fn forecast(community: &str, asset: &str, energy_kwh: f64) -> ForecastSchema {
    ForecastSchema {
        area_uuid: deterministic_area_uuid(community, asset),
        area_hash: h256_to_string(deterministic_area_hash(community, asset)),
        community_uuid: deterministic_community_uuid(community),
        time_slot: SLOT,
        creation_time: SLOT - 3600,
        energy_kwh,
        confidence: 1.0,
    }
}

fn garame_smart_meters() -> Vec<String> {
    (1..=7).map(|number| format!("GD{number:02}SM")).collect()
}

/// A positive demand forecast for every GD SmartMeter except `except`, plus a PV offer.
fn garame_forecasts(except: &[&str]) -> Vec<ForecastSchema> {
    let mut forecasts: Vec<ForecastSchema> = garame_smart_meters()
        .iter()
        .filter(|asset| !except.contains(&asset.as_str()))
        .map(|asset| forecast(GARAME, asset, 0.4))
        .collect();
    forecasts.push(forecast(GARAME, "GD01PV", -1.2));
    forecasts
}

// ---- the eligible list ---------------------------------------------------------------

#[test]
fn parse_community_list_of_the_default() {
    assert_eq!(
        parse_community_list(DEFAULT_INTER_COMMUNITY_ELIGIBLE_COMMUNITIES),
        vec![LUGAGGIA.to_string(), GARAME.to_string()]
    );
}

#[test]
fn parse_community_list_trims_and_skips_empty_entries() {
    assert_eq!(
        parse_community_list("  LugaggiaInnovationCommunity ,\tGaramèDistrict  "),
        vec![LUGAGGIA.to_string(), GARAME.to_string()]
    );
    assert_eq!(
        parse_community_list(",LugaggiaInnovationCommunity,, ,GaramèDistrict,"),
        vec![LUGAGGIA.to_string(), GARAME.to_string()]
    );
    assert!(parse_community_list("").is_empty());
    assert!(parse_community_list(" , ,").is_empty());
}

#[test]
fn parse_community_list_with_arena() {
    let spec = format!("{DEFAULT_INTER_COMMUNITY_ELIGIBLE_COMMUNITIES},{ARENA}");
    assert_eq!(
        parse_community_list(&spec),
        vec![LUGAGGIA.to_string(), GARAME.to_string(), ARENA.to_string()]
    );
}

#[test]
fn the_default_list_has_lugaggia_and_garame_but_not_arena() {
    assert_eq!(
        eligible_inter_communities(),
        parse_community_list(DEFAULT_INTER_COMMUNITY_ELIGIBLE_COMMUNITIES).as_slice()
    );
    assert!(eligible_inter_community(LUGAGGIA));
    assert!(eligible_inter_community(GARAME));
    assert!(!eligible_inter_community(ARENA));
    // The old per-LEC community name is not a site.
    assert!(!eligible_inter_community("Pilot2"));
}

// ---- demand_coverage -----------------------------------------------------------------

#[test]
fn every_smart_meter_forecast_is_full_coverage() {
    let coverage = demand_coverage(&live_set(), GARAME, &garame_forecasts(&[]));
    assert_eq!(
        coverage,
        DemandCoverage {
            expected: 7,
            missing: vec![],
        }
    );
    assert!(coverage.is_full());
}

#[test]
fn one_smart_meter_without_a_forecast_is_listed() {
    let coverage = demand_coverage(&live_set(), GARAME, &garame_forecasts(&["GD03SM"]));
    assert_eq!(coverage.expected, 7);
    assert_eq!(coverage.missing, vec!["GD03SM".to_string()]);
    assert!(!coverage.is_full());
}

#[test]
fn a_zero_or_negative_forecast_does_not_count() {
    let mut forecasts = garame_forecasts(&["GD03SM", "GD05SM"]);
    forecasts.push(forecast(GARAME, "GD03SM", 0.0));
    forecasts.push(forecast(GARAME, "GD05SM", -0.2));
    let coverage = demand_coverage(&live_set(), GARAME, &forecasts);
    assert_eq!(
        coverage.missing,
        vec!["GD03SM".to_string(), "GD05SM".to_string()]
    );
    assert!(!coverage.is_full());
}

#[test]
fn a_forecast_of_the_same_asset_in_another_community_does_not_count() {
    // The same SmartMeters under the old per-LEC ids.
    let forecasts: Vec<ForecastSchema> = garame_smart_meters()
        .iter()
        .map(|asset| forecast("Pilot2", asset, 0.4))
        .collect();
    let coverage = demand_coverage(&live_set(), GARAME, &forecasts);
    assert_eq!(coverage.expected, 7);
    assert_eq!(coverage.missing, garame_smart_meters());
}

#[test]
fn a_community_without_building_points_expects_nothing() {
    let set = live_set();
    for community in ["UrBeroaCommunity", "Pilot2", "NoSuchCommunity"] {
        let coverage = demand_coverage(&set, community, &garame_forecasts(&[]));
        assert_eq!(coverage.expected, 0, "{community}");
        assert!(coverage.missing.is_empty());
        assert!(!coverage.is_full());
    }
    let empty = demand_coverage(&MeteringPointSet::default(), GARAME, &garame_forecasts(&[]));
    assert_eq!(empty.expected, 0);
    assert!(!empty.is_full());
}

#[test]
fn arena_with_pv_forecasts_only_is_not_covered() {
    let set = live_set();
    let forecasts = vec![
        forecast(ARENA, "AIC01PV", -3.0),
        forecast(ARENA, "AIC44PV", -2.0),
    ];
    let coverage = demand_coverage(&set, ARENA, &forecasts);
    assert!(coverage.expected > 0);
    assert_eq!(coverage.missing.len(), coverage.expected);
    // Only building meters count: the site-level AIC44SM and AIC49SM are not expected.
    assert!(!coverage.missing.contains(&"AIC44SM".to_string()));
    assert!(!coverage.missing.contains(&"AIC49SM".to_string()));
    assert!(coverage.missing.contains(&"AIC34SM".to_string()));
}

#[test]
fn lugaggia_expects_the_smart_meter_of_every_building_meter() {
    let set = live_set();
    let expected: BTreeSet<String> = set
        .points
        .iter()
        .filter(|point| point.community_name == LUGAGGIA)
        .flat_map(|point| point.expected_meters.iter())
        .map(|token| format!("{token}SM"))
        .collect();
    assert!(!expected.contains("LIC02SM"));

    let forecasts: Vec<ForecastSchema> = expected
        .iter()
        .map(|asset| forecast(LUGAGGIA, asset, 0.3))
        .collect();
    let coverage = demand_coverage(&set, LUGAGGIA, &forecasts);
    assert_eq!(coverage.expected, expected.len());
    assert!(coverage.is_full());
}

#[test]
fn only_building_points_are_expected_in_a_hand_built_set() {
    let point = |name: &str, kind: MeteringPointKind, meters: &[&str]| MeteringPoint {
        community_name: GARAME.to_string(),
        community_uuid: deterministic_community_uuid(GARAME),
        name: name.to_string(),
        kind,
        area_uuid: deterministic_area_uuid(GARAME, name),
        area_hash: h256_to_string(deterministic_area_hash(GARAME, name)),
        members: BTreeSet::new(),
        member_area_hashes: vec![],
        expected_meters: meters.iter().map(|meter| meter.to_string()).collect(),
    };
    let set = MeteringPointSet {
        points: vec![
            point("TestHouse", MeteringPointKind::Building, &["GD01", "GD02"]),
            point("TestSite", MeteringPointKind::UnmeteredSite, &["GD03"]),
        ],
        notes: vec![],
        known_meter_tokens: BTreeSet::new(),
    };

    let forecasts = vec![
        forecast(GARAME, "GD01SM", 0.1),
        forecast(GARAME, "GD02SM", 0.2),
    ];
    assert_eq!(
        demand_coverage(&set, GARAME, &forecasts),
        DemandCoverage {
            expected: 2,
            missing: vec![],
        }
    );
    assert_eq!(
        demand_coverage(&set, GARAME, &forecasts[..1]).missing,
        vec!["GD02SM".to_string()]
    );
}
