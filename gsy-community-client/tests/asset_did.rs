//! Tests for the identity-server sync payload (plan §5.3).
//!
//! Two things are being guarded here, and neither is about HTTP:
//!
//! 1. **The `deterministic_areas()` extraction has not drifted.** `ingest_forecasts_loop`
//!    and `build_new_market_topology` used to derive `area_uuid`/`area_hash` from two
//!    separate copies of the same code. They now share one function, and the equivalence
//!    test below is what keeps the shared function honest — the ids it produces are the join
//!    keys for every forecast, order and market in the system, so a change here is a silent
//!    data-loss bug everywhere else.
//!
//! 2. **The cross-language wire contract with `gsy-ewf-identity-server`.** The serialisation
//!    tests assert against *literal* JSON, never against a round-trip through the same Rust
//!    structs: a round-trip is satisfied by any pair of matching Rust definitions, including
//!    one where both ends are wrong together, which is exactly the failure mode that matters
//!    when the other end of the wire is TypeScript.

use gsy_community_client::asset_did::{
    AssetDidClient, AssetSyncItem, AssetSyncRequest, AssetSyncResponse, CommunitySyncItem,
    SyncedSubject, build_sync_payload,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    build_new_market_topology, deterministic_area_hash, deterministic_area_uuid,
    deterministic_areas, deterministic_community_uuid,
};
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType};
use gsy_offchain_primitives::utils::h256_to_string;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::{Uuid, Version};

fn sample_community() -> ExternalCommunityTopology {
    ExternalCommunityTopology {
        community_name: "Pilot1".to_string(),
        areas: vec![
            ExternalAreaTopology {
                area_name: "LIC08SM".to_string(),
                area_type: AssetType::SMART_METER,
            },
            ExternalAreaTopology {
                area_name: "LIC03PV".to_string(),
                area_type: AssetType::PV,
            },
            ExternalAreaTopology {
                area_name: "LIC01BAT".to_string(),
                area_type: AssetType::BATTERY,
            },
            // §0.6 finding 5: an ontology asset type with no `AssetType` arm falls through
            // to AREA and still gets a DID — nothing is filtered (plan §2.5, Q8).
            ExternalAreaTopology {
                area_name: "LICSENSOR01".to_string(),
                area_type: AssetType::AREA,
            },
        ],
    }
}

fn second_community() -> ExternalCommunityTopology {
    ExternalCommunityTopology {
        community_name: "Pilot2".to_string(),
        areas: vec![ExternalAreaTopology {
            // Same asset name as nothing in Pilot1, but the community prefix is what makes
            // the derivation safe even if it were shared.
            area_name: "GD12HP".to_string(),
            area_type: AssetType::HEAT_PUMP,
        }],
    }
}

// ---------------------------------------------------------------------------------------
// 1. deterministic_areas() equivalence — guards the §4.4 extraction
// ---------------------------------------------------------------------------------------

#[test]
fn deterministic_areas_matches_build_new_market_topology() {
    let community = sample_community();

    let extracted = deterministic_areas(&community);
    let from_market = build_new_market_topology(&community, 1_700_000_000).community_areas;

    assert_eq!(
        extracted, from_market,
        "deterministic_areas() has drifted from build_new_market_topology(); every stored \
         forecast joins to a market area by area_uuid/area_hash, so the two MUST agree"
    );
}

#[test]
fn deterministic_areas_matches_the_underlying_helpers_field_by_field() {
    // Belt and braces: the test above compares two derivations that now share code, so it
    // would still pass if BOTH were rewritten wrongly. This one re-states the expected
    // values in terms of the primitive helpers `tests/identity.rs` already pins.
    let community = sample_community();

    for (area, source) in deterministic_areas(&community)
        .iter()
        .zip(community.areas.iter())
    {
        assert_eq!(area.name, source.area_name);
        assert_eq!(area.area_type, source.area_type);
        assert_eq!(
            area.area_uuid,
            deterministic_area_uuid(&community.community_name, &source.area_name)
        );
        assert_eq!(
            area.area_hash,
            h256_to_string(deterministic_area_hash(
                &community.community_name,
                &source.area_name
            ))
        );
    }
}

#[test]
fn deterministic_areas_preserves_order_and_arity() {
    let community = sample_community();
    let areas = deterministic_areas(&community);
    assert_eq!(areas.len(), community.areas.len());
    assert_eq!(
        areas.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
        community
            .areas
            .iter()
            .map(|a| a.area_name.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn deterministic_areas_of_an_empty_community_is_empty() {
    let community = ExternalCommunityTopology {
        community_name: "Pilot3".to_string(),
        areas: vec![],
    };
    assert!(deterministic_areas(&community).is_empty());
}

// ---------------------------------------------------------------------------------------
// 2. The cross-language contract with the TypeScript DTOs
// ---------------------------------------------------------------------------------------

/// Field list cross-checked by hand against
/// `gsy-ewf-identity-server/src/assets/dto/asset-sync-request.dto.ts` — `AssetSyncItem`
/// declares exactly `subjectUuid`, `assetName`, `assetType`, `communityName`,
/// `communityUuid`, `areaHash`, and the server runs `forbidNonWhitelisted: true`
/// (`src/main.ts:10-16`), so any extra or misspelled key 400s the whole sync.
#[test]
fn asset_sync_item_serialises_to_the_exact_dto_field_names() {
    let item = AssetSyncItem {
        subject_uuid: "1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d".to_string(),
        asset_name: "LIC08SM".to_string(),
        asset_type: AssetType::SMART_METER,
        community_name: "Pilot1".to_string(),
        community_uuid: "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c".to_string(),
        area_hash: "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0".to_string(),
    };

    let expected = json!({
        "subjectUuid": "1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d",
        "assetName": "LIC08SM",
        "assetType": "SMART_METER",
        "communityName": "Pilot1",
        "communityUuid": "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c",
        "areaHash": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0",
    });

    assert_eq!(serde_json::to_value(&item).unwrap(), expected);
}

/// Cross-checked against `CommunitySyncItem` in the same DTO file: exactly `subjectUuid`,
/// `communityName`, `communityUuid`.
#[test]
fn community_sync_item_serialises_to_the_exact_dto_field_names() {
    let item = CommunitySyncItem {
        subject_uuid: "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c".to_string(),
        community_name: "Pilot1".to_string(),
        community_uuid: "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c".to_string(),
    };

    let expected = json!({
        "subjectUuid": "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c",
        "communityName": "Pilot1",
        "communityUuid": "c7e0d0d3-0f8d-5a2b-9a5f-2f2c0f7a1b2c",
    });

    assert_eq!(serde_json::to_value(&item).unwrap(), expected);
}

#[test]
fn sync_items_emit_no_key_outside_the_dto_whitelist() {
    // `forbidNonWhitelisted: true` turns one stray key into a 400 for all 590 subjects, so
    // assert the key SETS explicitly rather than relying on the value comparisons above.
    let payload = build_sync_payload(&[sample_community()]);
    let value = serde_json::to_value(&payload).unwrap();

    assert_eq!(
        keys_of(&value),
        BTreeSet::from(["assets".to_string(), "communities".to_string()])
    );
    assert_eq!(
        keys_of(&value["assets"][0]),
        BTreeSet::from([
            "areaHash".to_string(),
            "assetName".to_string(),
            "assetType".to_string(),
            "communityName".to_string(),
            "communityUuid".to_string(),
            "subjectUuid".to_string(),
        ])
    );
    assert_eq!(
        keys_of(&value["communities"][0]),
        BTreeSet::from([
            "communityName".to_string(),
            "communityUuid".to_string(),
            "subjectUuid".to_string(),
        ])
    );
}

fn keys_of(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("expected a JSON object")
        .keys()
        .cloned()
        .collect()
}

/// `assetType` must be the *same* string the rest of the system already writes for
/// `AreaTopologySchema.area_type`, since the identity record is meant to be reconcilable
/// against the stored market topology. Asserted by serialising both and comparing, so a
/// serde attribute added to `AssetType` later fails here instead of silently splitting the
/// two spellings.
#[test]
fn asset_type_serialises_identically_to_area_topology_area_type() {
    let every_variant = [
        AssetType::BATTERY,
        AssetType::SMART_METER,
        AssetType::PV,
        AssetType::GRID_METER,
        AssetType::EV,
        AssetType::HEAT_PUMP,
        AssetType::BOILER,
        AssetType::AREA,
    ];

    for asset_type in every_variant {
        let area = AreaTopologySchema {
            area_uuid: "u".to_string(),
            name: "n".to_string(),
            area_type: asset_type.clone(),
            area_hash: "h".to_string(),
        };
        let item = AssetSyncItem {
            subject_uuid: "u".to_string(),
            asset_name: "n".to_string(),
            asset_type: asset_type.clone(),
            community_name: "c".to_string(),
            community_uuid: "cu".to_string(),
            area_hash: "h".to_string(),
        };

        assert_eq!(
            serde_json::to_value(&item).unwrap()["assetType"],
            serde_json::to_value(&area).unwrap()["area_type"],
        );
    }

    // And pin the actual spelling, so "both sides changed together" is also caught.
    assert_eq!(
        serde_json::to_value(AssetType::SMART_METER).unwrap(),
        json!("SMART_METER")
    );
    assert_eq!(serde_json::to_value(AssetType::PV).unwrap(), json!("PV"));
    assert_eq!(
        serde_json::to_value(AssetType::HEAT_PUMP).unwrap(),
        json!("HEAT_PUMP")
    );
    assert_eq!(
        serde_json::to_value(AssetType::AREA).unwrap(),
        json!("AREA")
    );
}

/// The server's `@IsUUID()` on `subjectUuid`/`communityUuid` rejects anything that is not a
/// well-formed UUID, and `@IsUUID()` with no argument accepts versions 3/4/5.
#[test]
fn every_subject_uuid_parses_as_a_v5_uuid() {
    let payload = build_sync_payload(&[sample_community(), second_community()]);

    for community in &payload.communities {
        assert_v5(&community.subject_uuid);
        assert_v5(&community.community_uuid);
    }
    for asset in &payload.assets {
        assert_v5(&asset.subject_uuid);
        assert_v5(&asset.community_uuid);
    }
}

fn assert_v5(candidate: &str) {
    let parsed = Uuid::parse_str(candidate)
        .unwrap_or_else(|err| panic!("{candidate} is not a parseable UUID: {err}"));
    assert_eq!(
        parsed.get_version(),
        Some(Version::Sha1),
        "{candidate} must be a v5 (SHA-1 name-based) UUID"
    );
}

// ---------------------------------------------------------------------------------------
// 3. Payload construction
// ---------------------------------------------------------------------------------------

#[test]
fn build_sync_payload_emits_one_community_and_one_asset_per_subject() {
    let communities = [sample_community(), second_community()];
    let payload = build_sync_payload(&communities);

    assert_eq!(payload.communities.len(), 2);
    assert_eq!(payload.assets.len(), 5);
    assert_eq!(payload.subject_count(), 7);
    assert!(!payload.is_empty());
}

#[test]
fn build_sync_payload_reuses_the_existing_deterministic_helpers() {
    let community = sample_community();
    let payload = build_sync_payload(std::slice::from_ref(&community));

    let expected_community_uuid = deterministic_community_uuid(&community.community_name);
    let item = &payload.communities[0];
    assert_eq!(item.subject_uuid, expected_community_uuid);
    assert_eq!(item.community_uuid, expected_community_uuid);
    assert_eq!(item.community_name, "Pilot1");

    // Asset subject ids ARE the market's area ids; that identity is the whole point of
    // keying the DID on `deterministic_area_uuid` (plan §2.2).
    let areas = deterministic_areas(&community);
    for (asset, area) in payload.assets.iter().zip(areas.iter()) {
        assert_eq!(asset.subject_uuid, area.area_uuid);
        assert_eq!(asset.area_hash, area.area_hash);
        assert_eq!(asset.asset_name, area.name);
        assert_eq!(asset.asset_type, area.area_type);
        assert_eq!(asset.community_uuid, expected_community_uuid);
        assert_eq!(asset.community_name, "Pilot1");
    }
}

#[test]
fn build_sync_payload_is_byte_identical_across_calls() {
    // Regenerability: the same ontology must always produce the same subjects, since the
    // identity server derives each DID from `subjectUuid` alone.
    let communities = [sample_community(), second_community()];
    assert_eq!(
        serde_json::to_string(&build_sync_payload(&communities)).unwrap(),
        serde_json::to_string(&build_sync_payload(&communities)).unwrap()
    );
}

#[test]
fn build_sync_payload_of_nothing_is_empty() {
    let payload = build_sync_payload(&[]);
    assert!(payload.is_empty());
    assert_eq!(payload.subject_count(), 0);
}

#[test]
fn a_community_with_no_assets_still_yields_a_community_subject() {
    let payload = build_sync_payload(&[ExternalCommunityTopology {
        community_name: "Pilot3".to_string(),
        areas: vec![],
    }]);
    assert_eq!(payload.communities.len(), 1);
    assert!(payload.assets.is_empty());
    assert!(!payload.is_empty());
}

// ---------------------------------------------------------------------------------------
// 4. Response parsing and the non-fatal error path
// ---------------------------------------------------------------------------------------

#[test]
fn sync_response_deserialises_from_the_servers_camel_case_body() {
    let body = json!({
        "created": 590,
        "updated": 0,
        "retired": 0,
        "subjects": [
            {
                "subjectUuid": "1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d",
                "subjectType": "asset",
                "did": "did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21",
                "registeredOnChain": false
            }
        ]
    });

    let response: AssetSyncResponse = serde_json::from_value(body).unwrap();
    assert_eq!(response.created, 590);
    assert_eq!(
        response.subjects,
        vec![SyncedSubject {
            subject_uuid: "1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d".to_string(),
            subject_type: "asset".to_string(),
            did: "did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21".to_string(),
            registered_on_chain: false,
        }]
    );
    assert_eq!(
        response
            .did_map()
            .get("1a2b3c4d-5e6f-5a7b-8c9d-0e1f2a3b4c5d"),
        Some(&"did:ethr:0xe194ab62fe7f8e28f2266b317bdcfc053999fb21".to_string())
    );
}

/// The sync loop's hard requirement (plan §2.4, R11): an identity server that is not there
/// produces an `Err` the caller logs, never a panic and never a stalled process. Pointed at
/// a port that was bound and immediately released, so nothing is listening on it.
#[tokio::test]
async fn sync_against_an_unreachable_server_returns_err_and_does_not_panic() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let client = AssetDidClient::new(
        Some(format!("http://127.0.0.1:{port}")),
        Some("fedecom_user".to_string()),
    );
    let payload = build_sync_payload(&[sample_community()]);

    let result = client.sync(&payload).await;
    assert!(
        result.is_err(),
        "an unreachable identity server must surface as Err, not as a success or a panic"
    );
}

#[tokio::test]
async fn sync_of_an_empty_payload_makes_no_request() {
    // Guards the loop against posting a body the server would 400 (its DTO rejects a
    // payload carrying neither communities nor assets). Points at a dead port, so any
    // request that WAS made would fail.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let client = AssetDidClient::new(
        Some(format!("http://127.0.0.1:{port}")),
        Some("fedecom_user".to_string()),
    );

    let response = client.sync(&AssetSyncRequest::default()).await.unwrap();
    assert_eq!(response, AssetSyncResponse::default());
}

#[test]
fn sync_url_is_the_documented_endpoint_and_tolerates_a_trailing_slash() {
    assert_eq!(
        AssetDidClient::new(
            Some("http://identity:3000".to_string()),
            Some("k".to_string())
        )
        .sync_url(),
        "http://identity:3000/asset-dids/sync"
    );
    assert_eq!(
        AssetDidClient::new(
            Some("http://identity:3000/".to_string()),
            Some("k".to_string())
        )
        .sync_url(),
        "http://identity:3000/asset-dids/sync"
    );
}
