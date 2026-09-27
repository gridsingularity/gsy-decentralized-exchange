use chrono::{DateTime, Utc};
use gsy_community_client::external_measurements::influxdb_api::InfluxMeasurementMeterData;
use gsy_community_client::external_measurements::metering_points::{
    MeterReadings, MeteringPoint, MeteringPointKind, MeteringPointNote, MeteringPointSet,
    build_metering_points, ended_slots, measurement_rows, parse_overrides, unmapped_tokens,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    deterministic_area_hash, deterministic_area_uuid, deterministic_community_uuid,
};
use gsy_community_client::topology::{LECCommunityAssetsResults, LECCommunityMembersResults};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema,
};
use gsy_offchain_primitives::utils::h256_to_string;
use std::collections::{BTreeSet, HashMap};
use std::ops::RangeInclusive;

// Live ontology responses (`get_lecs_buildings`, and `get_assets` per LEC), captured on
// 2026-09-26.
const LECS_BUILDINGS: &str = include_str!("fixtures/lecs_buildings.json");
const ASSETS_PILOT1: &str = include_str!("fixtures/assets_pilot1.json");
const ASSETS_PILOT2: &str = include_str!("fixtures/assets_pilot2.json");
const ASSETS_PILOT3: &str = include_str!("fixtures/assets_pilot3.json");

const SLOT_SEC: u64 = 900;
/// 2026-09-26T00:00:00Z.
const DAY_START: u64 = 1_790_380_800;
const GRACE_SEC: u64 = 165_600;

const LUGAGGIA: &str = "LugaggiaInnovationCommunity";
const GARAME: &str = "GaramèDistrict";
const ARENA: &str = "ArenaInnovationCommunity";

fn buildings() -> LECCommunityMembersResults {
    serde_json::from_str(LECS_BUILDINGS).expect("lecs_buildings.json parses")
}

fn assets(lec: &str, json: &str) -> (String, LECCommunityAssetsResults) {
    let parsed = serde_json::from_str(json).expect("assets fixture parses");
    (lec.to_string(), parsed)
}

fn all_assets() -> Vec<(String, LECCommunityAssetsResults)> {
    vec![
        assets("Pilot1", ASSETS_PILOT1),
        assets("Pilot2", ASSETS_PILOT2),
        assets("Pilot3", ASSETS_PILOT3),
    ]
}

fn live_set(overrides: &HashMap<String, String>) -> MeteringPointSet {
    build_metering_points(&buildings(), &all_assets(), overrides)
}

fn point<'a>(set: &'a MeteringPointSet, name: &str) -> &'a MeteringPoint {
    set.points
        .iter()
        .find(|point| point.name == name)
        .unwrap_or_else(|| panic!("no metering point {name}"))
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn tokens(prefix: &str, numbers: RangeInclusive<u32>) -> BTreeSet<String> {
    numbers
        .map(|number| format!("{prefix}{number:02}"))
        .collect()
}

fn hash(community: &str, name: &str) -> String {
    h256_to_string(deterministic_area_hash(community, name))
}

fn notes_where(
    set: &MeteringPointSet,
    keep: impl Fn(&MeteringPointNote) -> bool,
) -> Vec<MeteringPointNote> {
    set.notes
        .iter()
        .filter(|note| keep(note))
        .cloned()
        .collect()
}

fn site_level_meter(asset: &str, site: &str) -> MeteringPointNote {
    MeteringPointNote::SiteLevelMeter {
        community: site.to_string(),
        asset: asset.to_string(),
        token: asset.trim_end_matches("SM").to_string(),
        site: site.to_string(),
    }
}

fn member_without_own_meter(building: &str, asset: &str) -> MeteringPointNote {
    MeteringPointNote::MemberWithoutOwnMeter {
        community: ARENA.to_string(),
        building: building.to_string(),
        asset: asset.to_string(),
    }
}

// ---- build_metering_points on the live ontology -------------------------------------

#[test]
fn pilot2_has_a_point_per_building_and_two_unmetered_site_points() {
    let set = live_set(&HashMap::new());

    let pilot2_buildings: BTreeSet<String> = buildings()
        .results
        .bindings
        .iter()
        .filter(|row| row.lec_name.value == "Pilot2")
        .map(|row| row.participant_name.value.clone())
        .collect();
    let building_points: BTreeSet<String> = set
        .points
        .iter()
        .filter(|point| point.kind == MeteringPointKind::Building)
        .map(|point| point.name.clone())
        .collect();
    assert_eq!(pilot2_buildings.len(), 40);
    assert_eq!(building_points, pilot2_buildings);

    let site_points: Vec<&MeteringPoint> = set
        .points
        .iter()
        .filter(|point| point.kind == MeteringPointKind::UnmeteredSite)
        .collect();
    assert_eq!(site_points.len(), 2);
    assert_eq!(set.points.len(), 42);
    assert!(
        site_points
            .iter()
            .all(|point| point.expected_meters.is_empty())
    );

    assert_eq!(
        point(&set, "LugaggiaInnovationCommunity").members,
        names(&["LIC02DBATT", "LIC00SGIM", "LIC02SM", "LIC_transformer"])
    );
    assert_eq!(
        point(&set, "ArenaInnovationCommunity").members,
        names(&[
            "AIC49DBATT",
            "AIC44EV",
            "AIC00SGIM",
            "AIC44SM",
            "AIC49SM",
            "AIC_transformer",
        ])
    );
    assert!(set.points.iter().all(|point| point.name != GARAME));
}

#[test]
fn every_point_belongs_to_its_site() {
    let set = live_set(&HashMap::new());

    let site_of_building: HashMap<String, String> = buildings()
        .results
        .bindings
        .iter()
        .map(|row| {
            (
                row.participant_name.value.clone(),
                row.site_name.value.clone(),
            )
        })
        .collect();
    for point in &set.points {
        let site = match point.kind {
            MeteringPointKind::Building => site_of_building[&point.name].as_str(),
            MeteringPointKind::UnmeteredSite => point.name.as_str(),
        };
        assert_eq!(point.community_name, site, "{}", point.name);
    }

    let count = |site: &str, kind: MeteringPointKind| {
        set.points
            .iter()
            .filter(|point| point.community_name == site && point.kind == kind)
            .count()
    };
    assert_eq!(count(LUGAGGIA, MeteringPointKind::Building), 19);
    assert_eq!(count(LUGAGGIA, MeteringPointKind::UnmeteredSite), 1);
    assert_eq!(count(GARAME, MeteringPointKind::Building), 7);
    assert_eq!(count(GARAME, MeteringPointKind::UnmeteredSite), 0);
    assert_eq!(count(ARENA, MeteringPointKind::Building), 14);
    assert_eq!(count(ARENA, MeteringPointKind::UnmeteredSite), 1);
    assert_eq!(set.points.len(), 42);
}

#[test]
fn aic_house_11_expects_its_ten_meters() {
    let set = live_set(&HashMap::new());
    assert_eq!(
        point(&set, "AICHouse11").expected_meters,
        tokens("AIC", 34..=43)
    );
}

#[test]
fn lic_commercial_1_members_are_the_assets_located_there() {
    let set = live_set(&HashMap::new());
    let commercial = point(&set, "LICCommercial1");
    assert_eq!(
        commercial.members,
        names(&["LIC01SM", "LIC01PV", "LIC01HP", "LIC01EBOILER"])
    );
    assert_eq!(commercial.expected_meters, names(&["LIC01"]));
}

#[test]
fn without_overrides_aic44_is_split_between_the_site_and_aic_commercial_3() {
    let set = live_set(&HashMap::new());

    let commercial = point(&set, "AICCommercial3");
    assert_eq!(commercial.expected_meters, tokens("AIC", 45..=48));
    for asset in ["AIC44PV", "AIC44PV_1", "AIC44PV_2"] {
        assert!(
            commercial.members.contains(asset),
            "{asset} not in AICCommercial3"
        );
    }
    assert!(!commercial.members.contains("AIC44SM"));

    assert_eq!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::MemberWithoutOwnMeter { .. }
        )),
        vec![
            member_without_own_meter("AICCommercial3", "AIC44PV"),
            member_without_own_meter("AICCommercial3", "AIC44PV_1"),
            member_without_own_meter("AICCommercial3", "AIC44PV_2"),
        ]
    );
    assert_eq!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::SiteLevelMeter { .. }
        )),
        vec![
            site_level_meter("AIC44SM", ARENA),
            site_level_meter("AIC49SM", ARENA),
        ]
    );
    assert_eq!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::ExcludedMeter { .. }
        )),
        vec![MeteringPointNote::ExcludedMeter {
            community: LUGAGGIA.to_string(),
            asset: "LIC02SM".to_string(),
        }]
    );
    // Nothing else: every location resolves, every building has a meter.
    assert_eq!(set.notes.len(), 6, "unexpected notes: {:?}", set.notes);
}

#[test]
fn override_moves_aic44sm_into_aic_commercial_3() {
    let overrides = parse_overrides("AIC44SM=AICCommercial3").unwrap();
    let set = live_set(&overrides);

    let commercial = point(&set, "AICCommercial3");
    assert_eq!(commercial.expected_meters, tokens("AIC", 44..=48));
    assert!(commercial.members.contains("AIC44SM"));
    assert_eq!(
        point(&set, "ArenaInnovationCommunity").members,
        names(&[
            "AIC49DBATT",
            "AIC44EV",
            "AIC00SGIM",
            "AIC49SM",
            "AIC_transformer",
        ])
    );

    assert!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::MemberWithoutOwnMeter { .. }
        ))
        .is_empty()
    );
    assert_eq!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::SiteLevelMeter { .. }
        )),
        vec![site_level_meter("AIC49SM", ARENA)]
    );
    assert!(
        notes_where(&set, |note| matches!(
            note,
            MeteringPointNote::InvalidOverride { .. }
                | MeteringPointNote::UnknownOverrideAsset { .. }
        ))
        .is_empty()
    );
}

#[test]
fn override_to_anything_but_a_building_of_the_own_site_is_ignored() {
    // The asset's own site, a building of another site of the same LEC, a building of
    // another LEC, and an asset that does not exist.
    let overrides = parse_overrides(
        "AIC44SM=ArenaInnovationCommunity, LIC01PV=AICCommercial1, \
         AIC44EV=UrBeroaMainStation, NOSUCHASSET=AICHouse1",
    )
    .unwrap();
    let set = live_set(&overrides);

    assert_eq!(set.points, live_set(&HashMap::new()).points);
    for note in [
        MeteringPointNote::InvalidOverride {
            asset: "AIC44SM".to_string(),
            target: "ArenaInnovationCommunity".to_string(),
        },
        MeteringPointNote::InvalidOverride {
            asset: "LIC01PV".to_string(),
            target: "AICCommercial1".to_string(),
        },
        MeteringPointNote::InvalidOverride {
            asset: "AIC44EV".to_string(),
            target: "UrBeroaMainStation".to_string(),
        },
        MeteringPointNote::UnknownOverrideAsset {
            asset: "NOSUCHASSET".to_string(),
            target: "AICHouse1".to_string(),
        },
    ] {
        assert!(set.notes.contains(&note), "missing note {note:?}");
    }
}

#[test]
fn pilot1_and_pilot3_give_no_points() {
    let set = build_metering_points(
        &buildings(),
        &[
            assets("Pilot1", ASSETS_PILOT1),
            assets("Pilot3", ASSETS_PILOT3),
        ],
        &HashMap::new(),
    );
    assert!(set.points.is_empty());
    assert!(set.known_meter_tokens.is_empty());
    assert!(set.notes.is_empty(), "unexpected notes: {:?}", set.notes);
}

#[test]
fn known_meter_tokens_are_the_74_pilot2_smart_meters() {
    let set = live_set(&HashMap::new());

    let mut expected = tokens("AIC", 1..=49);
    expected.remove("AIC02");
    expected.remove("AIC11");
    expected.extend(tokens("GD", 1..=7));
    expected.extend(tokens("LIC", 1..=20));
    assert_eq!(expected.len(), 74);
    assert_eq!(set.known_meter_tokens, expected);
}

#[test]
fn ids_are_the_ones_markets_derive() {
    let set = live_set(&HashMap::new());

    for point in &set.points {
        let site = point.community_name.as_str();
        assert_eq!(point.community_uuid, deterministic_community_uuid(site));
        assert_eq!(point.area_uuid, deterministic_area_uuid(site, &point.name));
        assert_eq!(point.area_hash, hash(site, &point.name));
        let mut member_hashes: Vec<String> = point
            .members
            .iter()
            .map(|asset| hash(site, asset))
            .collect();
        member_hashes.sort();
        assert_eq!(point.member_area_hashes, member_hashes);
    }

    let commercial = point(&set, "LICCommercial1");
    assert_eq!(commercial.community_name, LUGAGGIA);
    assert_eq!(
        commercial.community_uuid,
        deterministic_community_uuid(LUGAGGIA)
    );
    assert_eq!(
        commercial.area_uuid,
        deterministic_area_uuid(LUGAGGIA, "LICCommercial1")
    );
    assert!(
        commercial
            .member_area_hashes
            .contains(&hash(LUGAGGIA, "LIC01PV"))
    );
    assert!(
        !commercial
            .member_area_hashes
            .contains(&hash("Pilot2", "LIC01PV"))
    );
    assert_eq!(point(&set, "GDHouse1").community_name, GARAME);
    assert_eq!(point(&set, ARENA).area_hash, hash(ARENA, ARENA));
}

#[test]
fn build_is_deterministic_and_independent_of_input_order() {
    let overrides = parse_overrides("AIC44SM=AICCommercial3").unwrap();
    let set = live_set(&overrides);
    assert_eq!(set, live_set(&overrides));

    let mut reversed_buildings = buildings();
    reversed_buildings.results.bindings.reverse();
    let mut reversed_assets = all_assets();
    reversed_assets.reverse();
    for (_, community_assets) in reversed_assets.iter_mut() {
        community_assets.results.bindings.reverse();
    }
    assert_eq!(
        build_metering_points(&reversed_buildings, &reversed_assets, &overrides),
        set
    );

    assert!(set.points.windows(2).all(|pair| {
        (&pair[0].community_name, &pair[0].name) < (&pair[1].community_name, &pair[1].name)
    }));
    assert!(set.notes.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(set.points.iter().all(|point| {
        point
            .member_area_hashes
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    }));
}

#[test]
fn notes_render_the_names_they_are_about() {
    let set = live_set(&HashMap::new());
    for note in &set.notes {
        let text = note.to_string();
        let (community, asset) = match note {
            MeteringPointNote::SiteLevelMeter {
                community, asset, ..
            }
            | MeteringPointNote::ExcludedMeter { community, asset }
            | MeteringPointNote::MemberWithoutOwnMeter {
                community, asset, ..
            } => (community, asset),
            other => panic!("unexpected note {other:?}"),
        };
        assert!(text.contains(asset.as_str()), "{text}");
        assert!(text.starts_with(&format!("{community}: ")), "{text}");
        assert!([LUGAGGIA, ARENA].contains(&community.as_str()), "{text}");
    }
}

// ---- parse_overrides ----------------------------------------------------------------

#[test]
fn parse_overrides_of_an_empty_or_blank_spec_is_empty() {
    assert!(parse_overrides("").unwrap().is_empty());
    assert!(parse_overrides("  \t ").unwrap().is_empty());
}

#[test]
fn parse_overrides_trims_every_entry() {
    let overrides = parse_overrides(" AIC44SM = AICCommercial3 ,AIC44EV=AICCommercial3, ").unwrap();
    assert_eq!(
        overrides,
        HashMap::from([
            ("AIC44SM".to_string(), "AICCommercial3".to_string()),
            ("AIC44EV".to_string(), "AICCommercial3".to_string()),
        ])
    );
}

#[test]
fn parse_overrides_rejects_a_malformed_entry_by_name() {
    for (spec, entry) in [
        ("AIC44SM=AICCommercial3,AIC44EV", "AIC44EV"),
        ("=AICCommercial3", "=AICCommercial3"),
        ("AIC44SM=", "AIC44SM="),
        (
            "AIC44SM=AICCommercial3=AICHouse1",
            "AIC44SM=AICCommercial3=AICHouse1",
        ),
    ] {
        let error = parse_overrides(spec).unwrap_err();
        assert!(error.contains(&format!("'{entry}'")), "{spec}: {error}");
    }
}

#[test]
fn parse_overrides_rejects_two_buildings_for_one_asset() {
    let error = parse_overrides("AIC44SM=AICCommercial3,AIC44SM=AICHouse1").unwrap_err();
    assert!(error.contains("AIC44SM=AICHouse1"), "{error}");
    assert_eq!(
        parse_overrides("AIC44SM=AICCommercial3,AIC44SM=AICCommercial3").unwrap(),
        HashMap::from([("AIC44SM".to_string(), "AICCommercial3".to_string())])
    );
}

// ---- ended_slots --------------------------------------------------------------------

#[test]
fn ended_slots_starts_at_the_first_slot_inside_the_window_and_skips_the_running_one() {
    // 07:00 into the slot starting at DAY_START + 10h.
    let now = DAY_START + 10 * 3600 + 420;
    assert_eq!(
        ended_slots(now, 3600, SLOT_SEC),
        vec![
            DAY_START + 9 * 3600 + 900,
            DAY_START + 9 * 3600 + 1800,
            DAY_START + 9 * 3600 + 2700,
        ]
    );
}

#[test]
fn ended_slots_on_a_slot_boundary_includes_both_ends_of_the_window() {
    let now = DAY_START + 10 * 3600;
    assert_eq!(
        ended_slots(now, 3600, SLOT_SEC),
        vec![
            DAY_START + 9 * 3600,
            DAY_START + 9 * 3600 + 900,
            DAY_START + 9 * 3600 + 1800,
            DAY_START + 9 * 3600 + 2700,
        ]
    );
    // A 50 h look-back lists every one of its 200 slots, data or not.
    let slots = ended_slots(now, 180_000, SLOT_SEC);
    assert_eq!(slots.len(), 200);
    assert_eq!(slots.first(), Some(&(now - 180_000)));
    assert_eq!(slots.last(), Some(&(now - SLOT_SEC)));
    assert!(slots.windows(2).all(|pair| pair[1] == pair[0] + SLOT_SEC));
}

#[test]
fn ended_slots_edge_cases() {
    // The window is clamped at 0.
    assert_eq!(ended_slots(2000, 10_000, SLOT_SEC), vec![0, 900]);
    // No slot has ended inside an empty window.
    assert!(ended_slots(DAY_START, 0, SLOT_SEC).is_empty());
    assert!(ended_slots(DAY_START + 899, 899, SLOT_SEC).is_empty());
    assert!(ended_slots(DAY_START, 3600, 0).is_empty());
}

// ---- measurement_rows / unmapped_tokens ---------------------------------------------

const TEST_COMMUNITY: &str = "TestCommunity";

fn test_point(
    name: &str,
    kind: MeteringPointKind,
    members: &[&str],
    meters: &[&str],
) -> MeteringPoint {
    let members = names(members);
    let mut member_area_hashes: Vec<String> = members
        .iter()
        .map(|asset| hash(TEST_COMMUNITY, asset))
        .collect();
    member_area_hashes.sort();
    MeteringPoint {
        community_name: TEST_COMMUNITY.to_string(),
        community_uuid: deterministic_community_uuid(TEST_COMMUNITY),
        name: name.to_string(),
        kind,
        area_uuid: deterministic_area_uuid(TEST_COMMUNITY, name),
        area_hash: hash(TEST_COMMUNITY, name),
        members,
        member_area_hashes,
        expected_meters: names(meters),
    }
}

/// A building with two meters, `AIC01` and `AIC03`.
fn house() -> MeteringPoint {
    test_point(
        "TestHouse",
        MeteringPointKind::Building,
        &["AIC01SM", "AIC03SM", "AIC03PV"],
        &["AIC01", "AIC03"],
    )
}

fn site() -> MeteringPoint {
    test_point(
        "TestSite",
        MeteringPointKind::UnmeteredSite,
        &["TEST_transformer"],
        &[],
    )
}

fn slot(index: u64) -> u64 {
    DAY_START + index * SLOT_SEC
}

fn add_reading(
    readings: &mut MeterReadings,
    token: &str,
    slot: u64,
    import_wh: Option<f64>,
    export_wh: Option<f64>,
) {
    let time = DateTime::<Utc>::from_timestamp(slot as i64, 0).unwrap();
    readings.entry(token.to_string()).or_default().insert(
        time,
        InfluxMeasurementMeterData {
            sensor_id: token.to_string(),
            time,
            import_Wh: import_wh,
            export_Wh: export_wh,
        },
    );
}

/// Slot 0: both meters, net export. Slot 1: both meters, net import. Slot 2: `AIC03` absent.
/// Slot 3: `AIC03` has `import` only. Slot 4: no data at all. `AIC11`, which no point
/// expects, reports in every slot.
fn readings() -> MeterReadings {
    let mut readings = MeterReadings::new();
    add_reading(&mut readings, "AIC01", slot(0), Some(1500.0), Some(250.0));
    add_reading(&mut readings, "AIC03", slot(0), Some(0.0), Some(4578.0));
    add_reading(&mut readings, "AIC01", slot(1), Some(2000.0), Some(0.0));
    add_reading(&mut readings, "AIC03", slot(1), Some(500.0), Some(1000.0));
    add_reading(&mut readings, "AIC01", slot(2), Some(1000.0), Some(0.0));
    add_reading(&mut readings, "AIC01", slot(3), Some(200.0), Some(0.0));
    add_reading(&mut readings, "AIC03", slot(3), Some(300.0), None);
    for index in 0..5 {
        add_reading(&mut readings, "AIC11", slot(index), Some(7000.0), Some(0.0));
    }
    readings
}

fn completeness(row: &MeasurementSchema) -> MeasurementCompleteness {
    row.metering_point.as_ref().unwrap().completeness.clone()
}

fn missing_meters(row: &MeasurementSchema) -> Vec<String> {
    row.metering_point.as_ref().unwrap().missing_meters.clone()
}

#[test]
fn complete_slots_sum_every_meter_in_kwh_as_net_import() {
    let house = house();
    // Right after slot 1 ended: long before the grace period, complete rows are posted.
    let now = slot(2);
    let rows = measurement_rows(
        std::slice::from_ref(&house),
        &readings(),
        &[slot(0), slot(1)],
        now,
        GRACE_SEC,
    );
    assert_eq!(rows.len(), 2);

    // (1500 - 250) / 1000 + (0 - 4578) / 1000 = 1.25 - 4.578: a net export.
    assert!((rows[0].energy_kwh - (1.25 - 4.578)).abs() < 1e-9);
    // (2000 - 0) / 1000 + (500 - 1000) / 1000 = 2.0 - 0.5: a net import.
    assert!((rows[1].energy_kwh - 1.5).abs() < 1e-9);

    for (row, expected_slot) in rows.iter().zip([slot(0), slot(1)]) {
        assert_eq!(completeness(row), MeasurementCompleteness::Complete);
        assert!(missing_meters(row).is_empty());
        assert_eq!(row.time_slot, expected_slot);
        assert_eq!(row.creation_time, now);
        assert_eq!(row.area_uuid, house.area_uuid);
        assert_eq!(row.area_hash, house.area_hash);
        assert_eq!(row.community_uuid, house.community_uuid);
        let metering_point = row.metering_point.as_ref().unwrap();
        assert_eq!(metering_point.name, "TestHouse");
        assert_eq!(metering_point.member_area_hashes, house.member_area_hashes);
    }
}

#[test]
fn a_slot_with_an_absent_meter_waits_for_the_grace_period_then_is_incomplete() {
    let points = [house()];
    let readings = readings();

    let before = measurement_rows(
        &points,
        &readings,
        &[slot(2)],
        slot(2) + GRACE_SEC - 1,
        GRACE_SEC,
    );
    assert!(before.is_empty());

    let after = measurement_rows(
        &points,
        &readings,
        &[slot(2)],
        slot(2) + GRACE_SEC,
        GRACE_SEC,
    );
    assert_eq!(after.len(), 1);
    assert_eq!(completeness(&after[0]), MeasurementCompleteness::Incomplete);
    assert_eq!(missing_meters(&after[0]), vec!["AIC03".to_string()]);
    assert_eq!(after[0].energy_kwh, 0.0);
}

#[test]
fn a_meter_with_import_but_no_export_makes_the_slot_incomplete() {
    let points = [house()];
    let readings = readings();

    let before = measurement_rows(&points, &readings, &[slot(3)], slot(4), GRACE_SEC);
    assert!(before.is_empty());

    let after = measurement_rows(
        &points,
        &readings,
        &[slot(3)],
        slot(3) + GRACE_SEC,
        GRACE_SEC,
    );
    assert_eq!(after.len(), 1);
    assert_eq!(completeness(&after[0]), MeasurementCompleteness::Incomplete);
    assert_eq!(missing_meters(&after[0]), vec!["AIC03".to_string()]);
    assert_eq!(after[0].energy_kwh, 0.0);
}

#[test]
fn a_point_whose_only_meter_lacks_export_is_missing() {
    // No expected meter has a value, so nothing reported for the slot.
    let points = [test_point(
        "TestFlat",
        MeteringPointKind::Building,
        &["AIC03SM"],
        &["AIC03"],
    )];
    let rows = measurement_rows(
        &points,
        &readings(),
        &[slot(3)],
        slot(3) + GRACE_SEC,
        GRACE_SEC,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(completeness(&rows[0]), MeasurementCompleteness::Missing);
    assert_eq!(missing_meters(&rows[0]), vec!["AIC03".to_string()]);
}

#[test]
fn a_slot_without_any_data_is_missing_after_the_grace_period() {
    let points = [house()];
    let readings = readings();

    assert!(measurement_rows(&points, &readings, &[slot(4)], slot(5), GRACE_SEC).is_empty());

    let after = measurement_rows(
        &points,
        &readings,
        &[slot(4)],
        slot(4) + GRACE_SEC,
        GRACE_SEC,
    );
    assert_eq!(after.len(), 1);
    assert_eq!(completeness(&after[0]), MeasurementCompleteness::Missing);
    assert_eq!(
        missing_meters(&after[0]),
        vec!["AIC01".to_string(), "AIC03".to_string()]
    );
    assert_eq!(after[0].energy_kwh, 0.0);
}

#[test]
fn an_unmetered_point_is_missing_for_every_slot_after_the_grace_period() {
    let points = [site()];
    let readings = readings();
    let slots: Vec<u64> = (0..5).map(slot).collect();

    assert!(measurement_rows(&points, &readings, &slots, slot(5), GRACE_SEC).is_empty());

    let rows = measurement_rows(&points, &readings, &slots, slot(4) + GRACE_SEC, GRACE_SEC);
    assert_eq!(rows.len(), 5);
    for (row, expected_slot) in rows.iter().zip(slots) {
        assert_eq!(row.time_slot, expected_slot);
        assert_eq!(completeness(row), MeasurementCompleteness::Missing);
        assert!(missing_meters(row).is_empty());
        assert_eq!(row.energy_kwh, 0.0);
        assert_eq!(row.metering_point.as_ref().unwrap().name, "TestSite");
    }
}

#[test]
fn a_slot_that_has_not_started_is_never_missing() {
    let rows = measurement_rows(&[site(), house()], &readings(), &[slot(6)], slot(5), 0);
    assert!(rows.is_empty());
}

#[test]
fn readings_of_an_unknown_token_are_ignored_and_reported_as_unmapped() {
    let points = [house(), site()];
    let slots: Vec<u64> = (0..5).map(slot).collect();
    let now = slot(4) + GRACE_SEC;

    let mut without_aic11 = readings();
    without_aic11.remove("AIC11");
    assert_eq!(
        measurement_rows(&points, &readings(), &slots, now, GRACE_SEC),
        measurement_rows(&points, &without_aic11, &slots, now, GRACE_SEC)
    );

    let set = MeteringPointSet {
        points: points.to_vec(),
        notes: vec![],
        known_meter_tokens: names(&["AIC01", "AIC03"]),
    };
    assert_eq!(unmapped_tokens(&set, &readings()), names(&["AIC11"]));
    assert!(unmapped_tokens(&set, &without_aic11).is_empty());

    // On the live ontology, AIC11 is the only meter without a SmartMeter asset.
    let mut live_readings = readings();
    add_reading(&mut live_readings, "AIC44", slot(0), Some(1.0), Some(0.0));
    add_reading(&mut live_readings, "LIC02", slot(0), Some(1.0), Some(0.0));
    assert_eq!(
        unmapped_tokens(&live_set(&HashMap::new()), &live_readings),
        names(&["AIC11"])
    );
}

#[test]
fn measurement_rows_are_deterministic() {
    let points = [house(), site()];
    let slots: Vec<u64> = (0..5).map(slot).collect();
    let now = slot(4) + GRACE_SEC;

    let rows = measurement_rows(&points, &readings(), &slots, now, GRACE_SEC);
    assert_eq!(
        rows,
        measurement_rows(&points, &readings(), &slots, now, GRACE_SEC)
    );

    // Point by point, slot by slot.
    let order: Vec<(String, u64)> = rows
        .iter()
        .map(|row| {
            (
                row.metering_point.as_ref().unwrap().name.clone(),
                row.time_slot,
            )
        })
        .collect();
    let expected: Vec<(String, u64)> = ["TestHouse", "TestSite"]
        .iter()
        .flat_map(|name| slots.iter().map(move |&slot| (name.to_string(), slot)))
        .collect();
    assert_eq!(order, expected);
}
