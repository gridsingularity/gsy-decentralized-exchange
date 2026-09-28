//! Per-site communities: every ontology site is its own community and spot market, while
//! asset DIDs stay keyed on the LEC. All tests run on the live ontology fixtures.

use gsy_community_client::asset_did::build_sync_payload;
use gsy_community_client::external_measurements::metering_points::build_metering_points;
use gsy_community_client::offchain_storage_connector::adapter::{
    AreaMarketInfoAdapter, build_new_market_topology, deterministic_area_uuid, deterministic_areas,
    deterministic_community_uuid,
};
use gsy_community_client::sites::{SiteIndex, location_fragment};
use gsy_community_client::topology::{
    ExternalCommunityTopology, LECCommunityAssetsResults, LECCommunityMembersResults, RawOntology,
    TopologyManager,
};
use reqwest::Client;
use std::collections::{BTreeMap, HashMap};

// Live ontology responses (`get_lecs_buildings`, and `get_assets` per LEC), captured on
// 2026-09-26.
const LECS_BUILDINGS: &str = include_str!("fixtures/lecs_buildings.json");
const ASSETS_PILOT1: &str = include_str!("fixtures/assets_pilot1.json");
const ASSETS_PILOT2: &str = include_str!("fixtures/assets_pilot2.json");
const ASSETS_PILOT3: &str = include_str!("fixtures/assets_pilot3.json");

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

fn raw() -> RawOntology {
    RawOntology {
        buildings: buildings(),
        assets: vec![
            assets("Pilot1", ASSETS_PILOT1),
            assets("Pilot2", ASSETS_PILOT2),
            assets("Pilot3", ASSETS_PILOT3),
        ],
    }
}

// Makes no network call.
fn manager() -> TopologyManager {
    TopologyManager::new(&Client::new(), &AreaMarketInfoAdapter::new(None))
}

fn sizes(communities: &[ExternalCommunityTopology]) -> Vec<(String, usize)> {
    communities
        .iter()
        .map(|community| (community.community_name.clone(), community.areas.len()))
        .collect()
}

/// Every asset name with the number of communities that hold it.
fn asset_counts(communities: &[ExternalCommunityTopology]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for community in communities {
        for area in &community.areas {
            *counts.entry(area.area_name.clone()).or_insert(0) += 1;
        }
    }
    counts
}

// ---- SiteIndex ----------------------------------------------------------------------

#[test]
fn location_fragment_is_the_part_after_the_hash() {
    assert_eq!(
        location_fragment("https://w3id.org/fedecom/characterization-main#LICHouse1"),
        "LICHouse1"
    );
    assert_eq!(location_fragment("LICHouse1"), "LICHouse1");
    assert_eq!(location_fragment("a#b#GDHouse2"), "GDHouse2");
}

#[test]
fn a_building_belongs_to_its_site_and_a_site_to_itself() {
    let index = SiteIndex::new(&buildings());

    assert_eq!(index.building_site("Pilot2", "LICHouse1"), Some(LUGAGGIA));
    assert_eq!(index.building_site("Pilot2", "GDHouse1"), Some(GARAME));
    assert_eq!(index.building_site("Pilot2", "AICCommercial1"), Some(ARENA));
    // Buildings and sites are looked up within their own LEC only.
    assert_eq!(index.building_site("Pilot1", "LICHouse1"), None);
    assert_eq!(index.building_site("Pilot2", ARENA), None);

    assert_eq!(
        index.site_of_location("Pilot2", "LICHouse1"),
        Some(LUGAGGIA)
    );
    assert_eq!(index.site_of_location("Pilot2", ARENA), Some(ARENA));
    assert_eq!(index.site_of_location("Pilot1", ARENA), None);
    assert_eq!(index.site_of_location("Pilot2", "Pilot2"), None);
    assert_eq!(index.site_of_location("Pilot2", "NoSuchBuilding"), None);
    assert!(index.is_site("Pilot2", GARAME));
    assert!(!index.is_site("Pilot3", GARAME));

    assert_eq!(index.buildings_of_site("Pilot2", LUGAGGIA).len(), 19);
    assert_eq!(index.buildings_of_site("Pilot2", GARAME).len(), 7);
    assert_eq!(index.buildings_of_site("Pilot2", ARENA).len(), 14);
    assert!(index.buildings_of_site("Pilot1", ARENA).is_empty());
}

#[test]
fn the_site_index_does_not_depend_on_the_row_order() {
    let mut reversed = buildings();
    reversed.results.bindings.reverse();
    assert_eq!(SiteIndex::new(&reversed), SiteIndex::new(&buildings()));
}

// ---- communities_by_site / communities_by_lec -----------------------------------------

#[test]
fn communities_by_site_are_the_nine_sites_sorted_by_name() {
    let communities = manager().communities_by_site(&raw());

    assert_eq!(
        sizes(&communities),
        vec![
            (ARENA.to_string(), 68),
            ("Brico_HQ".to_string(), 12),
            ("ENBRO_Community".to_string(), 268),
            ("EZ_Barcelona_TMB".to_string(), 2),
            ("EZ_Puertollano".to_string(), 3),
            (GARAME.to_string(), 25),
            (LUGAGGIA.to_string(), 93),
            ("TownHall".to_string(), 61),
            ("UrBeroaCommunity".to_string(), 55),
        ]
    );
}

#[test]
fn communities_by_lec_are_the_three_lecs() {
    let communities = manager().communities_by_lec(&raw());
    assert_eq!(
        sizes(&communities),
        vec![
            ("Pilot1".to_string(), 121),
            ("Pilot2".to_string(), 186),
            ("Pilot3".to_string(), 280),
        ]
    );
}

#[test]
fn splitting_by_site_loses_no_asset_and_keeps_every_type() {
    let manager = manager();
    let by_site = manager.communities_by_site(&raw());
    let by_lec = manager.communities_by_lec(&raw());

    let site_counts = asset_counts(&by_site);
    assert_eq!(site_counts.len(), 587);
    assert!(site_counts.values().all(|&count| count == 1));
    assert_eq!(site_counts, asset_counts(&by_lec));

    let types = |communities: &[ExternalCommunityTopology]| {
        communities
            .iter()
            .flat_map(|community| community.areas.iter())
            .map(|area| (area.area_name.clone(), area.area_type.clone()))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(types(&by_site), types(&by_lec));
}

#[test]
fn every_asset_is_in_the_site_of_its_location() {
    let index = SiteIndex::new(&buildings());
    let by_site = manager().communities_by_site(&raw());
    let community_of: HashMap<String, String> = by_site
        .iter()
        .flat_map(|community| {
            community
                .areas
                .iter()
                .map(|area| (area.area_name.clone(), community.community_name.clone()))
        })
        .collect();

    for (lec, lec_assets) in raw().assets {
        for asset in &lec_assets.results.bindings {
            let site = index
                .site_of_asset(&lec, asset)
                .expect("every live asset has a site");
            assert_eq!(community_of[&asset.asset_name.value], site);
        }
    }
    // The two site-level assets checked by hand.
    assert_eq!(community_of["LIC02SM"], LUGAGGIA);
    assert_eq!(community_of["AIC44SM"], ARENA);
    assert_eq!(community_of["AIC44PV"], ARENA);
    assert_eq!(community_of["GD01SM"], GARAME);
}

#[test]
fn an_asset_without_a_site_is_left_out() {
    let mut raw = raw();
    let (_, pilot2) = raw
        .assets
        .iter_mut()
        .find(|(lec, _)| lec == "Pilot2")
        .unwrap();
    let mut stray = pilot2.results.bindings[0].clone();
    stray.asset_name.value = "STRAY01SM".to_string();
    stray.location.value = "https://w3id.org/fedecom/characterization-main#Nowhere".to_string();
    pilot2.results.bindings.push(stray);

    let communities = manager().communities_by_site(&raw);
    assert_eq!(asset_counts(&communities).len(), 587);
    assert!(!asset_counts(&communities).contains_key("STRAY01SM"));
    assert_eq!(communities.len(), 9);
}

#[test]
fn site_markets_derive_their_ids_from_the_site() {
    let by_site = manager().communities_by_site(&raw());
    let lugaggia = by_site
        .iter()
        .find(|community| community.community_name == LUGAGGIA)
        .unwrap();

    let market = build_new_market_topology(lugaggia, 1_790_380_800);
    assert_eq!(market.community_name, LUGAGGIA);
    assert_eq!(
        market.community_uuid,
        deterministic_community_uuid(LUGAGGIA)
    );
    let lic01sm = market
        .community_areas
        .iter()
        .find(|area| area.name == "LIC01SM")
        .unwrap();
    assert_eq!(
        lic01sm.area_uuid,
        deterministic_area_uuid(LUGAGGIA, "LIC01SM")
    );
}

// ---- metering points agree with the markets -----------------------------------------------

#[test]
fn every_metering_point_member_hash_is_its_market_area_hash() {
    let raw = raw();
    let by_site = manager().communities_by_site(&raw);
    let set = build_metering_points(&raw.buildings, &raw.assets, &HashMap::new());
    assert_eq!(set.points.len(), 42);

    for point in &set.points {
        let community = by_site
            .iter()
            .find(|community| community.community_name == point.community_name)
            .unwrap_or_else(|| panic!("no market community {}", point.community_name));
        let market_hashes: HashMap<String, String> = deterministic_areas(community)
            .into_iter()
            .map(|area| (area.name, area.area_hash))
            .collect();

        let mut expected: Vec<String> = point
            .members
            .iter()
            .map(|member| {
                market_hashes
                    .get(member)
                    .unwrap_or_else(|| panic!("{member} not in market {}", point.community_name))
                    .clone()
            })
            .collect();
        expected.sort();
        assert_eq!(point.member_area_hashes, expected, "{}", point.name);
        assert_eq!(
            point.community_uuid,
            deterministic_community_uuid(&community.community_name)
        );
    }
}

// ---- DIDs stay per LEC -------------------------------------------------------------------

#[test]
fn did_subjects_stay_keyed_on_the_lec() {
    let payload = build_sync_payload(&manager().communities_by_lec(&raw()));

    let community_subjects: Vec<(String, String)> = payload
        .communities
        .iter()
        .map(|community| {
            (
                community.community_name.clone(),
                community.subject_uuid.clone(),
            )
        })
        .collect();
    assert_eq!(
        community_subjects,
        ["Pilot1", "Pilot2", "Pilot3"]
            .iter()
            .map(|lec| (lec.to_string(), deterministic_community_uuid(lec)))
            .collect::<Vec<_>>()
    );

    let lic01sm = payload
        .assets
        .iter()
        .find(|asset| asset.asset_name == "LIC01SM")
        .unwrap();
    assert_eq!(
        lic01sm.subject_uuid,
        deterministic_area_uuid("Pilot2", "LIC01SM")
    );
    assert_eq!(lic01sm.community_name, "Pilot2");
    assert_ne!(
        lic01sm.subject_uuid,
        deterministic_area_uuid(LUGAGGIA, "LIC01SM")
    );
    assert_eq!(payload.assets.len(), 587);
}
