use gsy_community_client::offchain_storage_connector::adapter::{
    deterministic_area_hash, deterministic_area_uuid, deterministic_community_uuid,
};
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string};

#[test]
fn deterministic_community_uuid_is_stable_across_calls() {
    assert_eq!(
        deterministic_community_uuid("LugaggiaInnovationCommunity"),
        deterministic_community_uuid("LugaggiaInnovationCommunity")
    );
}

#[test]
fn deterministic_community_uuid_differs_across_communities() {
    assert_ne!(
        deterministic_community_uuid("LugaggiaInnovationCommunity"),
        deterministic_community_uuid("GaramèDistrict")
    );
}

#[test]
fn deterministic_area_uuid_is_stable_across_calls() {
    assert_eq!(
        deterministic_area_uuid("LugaggiaInnovationCommunity", "LIC08SM"),
        deterministic_area_uuid("LugaggiaInnovationCommunity", "LIC08SM")
    );
}

#[test]
fn deterministic_area_uuid_differs_across_areas_within_a_community() {
    assert_ne!(
        deterministic_area_uuid("LugaggiaInnovationCommunity", "LIC08SM"),
        deterministic_area_uuid("LugaggiaInnovationCommunity", "LIC03PV")
    );
}

#[test]
fn deterministic_area_uuid_differs_across_communities_for_the_same_area_name() {
    // Guards against a naive derivation that only hashes the area name.
    assert_ne!(
        deterministic_area_uuid("LugaggiaInnovationCommunity", "SM01"),
        deterministic_area_uuid("GaramèDistrict", "SM01")
    );
}

#[test]
fn deterministic_area_hash_is_stable_across_calls() {
    assert_eq!(
        deterministic_area_hash("LugaggiaInnovationCommunity", "LIC08SM"),
        deterministic_area_hash("LugaggiaInnovationCommunity", "LIC08SM")
    );
}

#[test]
fn deterministic_area_hash_differs_across_areas() {
    assert_ne!(
        deterministic_area_hash("LugaggiaInnovationCommunity", "LIC08SM"),
        deterministic_area_hash("LugaggiaInnovationCommunity", "LIC03PV")
    );
}

#[test]
fn community_id_from_deterministic_community_uuid_is_stable() {
    let community_uuid = deterministic_community_uuid("GaramèDistrict");
    assert_eq!(
        h256_to_string(community_id_from_uuid(&community_uuid)),
        h256_to_string(community_id_from_uuid(&community_uuid))
    );
}
