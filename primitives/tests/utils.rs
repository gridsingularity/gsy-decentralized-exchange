use primitives::utils::{
    bytes16_to_hex, bytes16_to_uuid_string, derive_onchain_id, normalize_uuid,
    normalize_offchain_id, hex_to_bytes16, uuid_v4_to_bytes16,
};

#[test]
fn identity_derivation_preserves_all_actor_spellings() {
    let lower = "abcdefab-1234-1234-1234-abcdefabcdef";
    assert_ne!(
        derive_onchain_id(&lower.to_uppercase()),
        derive_onchain_id(lower)
    );
    for opaque in [
        "Facility-A",
        "0xABCDEFAB123412341234ABCDEFABCDEF",
        "ABCDEFAB123412341234ABCDEFABCDEF",
    ] {
        assert_ne!(
            derive_onchain_id(opaque),
            derive_onchain_id(&opaque.to_lowercase())
        );
    }
    for kind in [
        primitives::db_api_schema::profiles::MeasurementPointType::Measurement,
        primitives::db_api_schema::profiles::MeasurementPointType::Forecast,
    ] {
        assert_ne!(
            primitives::utils::profile_measurement_id(kind.clone(), "community", lower),
            primitives::utils::profile_measurement_id(kind, "community", &lower.to_uppercase())
        );
    }
}

#[test]
fn profile_ids_preserve_exact_community_references() {
    use primitives::db_api_schema::profiles::MeasurementPointType;
    use primitives::utils::profile_measurement_id;
    let community = "abcdefab-1234-1234-1234-abcdefabcdef";
    for kind in [
        MeasurementPointType::Measurement,
        MeasurementPointType::Forecast,
    ] {
        assert_ne!(
            profile_measurement_id(kind.clone(), community, "facility-a"),
            profile_measurement_id(kind, &community.to_uppercase(), "facility-a")
        );
    }
}

#[test]
fn onchain_identity_derivation_is_keyed() {
    let hasher = primitives::actor_ids::ActorIdHasher::from_hex_key(
        &std::env::var("ACTOR_ID_HASH_KEY").unwrap(),
    )
    .unwrap();
    assert_eq!(derive_onchain_id("facility-a"), hasher.derive("facility-a"));
    assert_ne!(
        primitives::utils::derive_onchain_id("facility-a"),
        primitives::utils::derive_onchain_id("Facility-a")
    );
}

#[test]
fn profile_ids_do_not_alias_opaque_components_with_separators() {
    use primitives::db_api_schema::profiles::MeasurementPointType;
    use primitives::utils::profile_measurement_id;
    for kind in [
        MeasurementPointType::Measurement,
        MeasurementPointType::Forecast,
    ] {
        let ids = [
            profile_measurement_id(kind.clone(), "A:B", "C"),
            profile_measurement_id(kind.clone(), "A", "B:C"),
            profile_measurement_id(kind.clone(), "A%3AB", "C"),
        ];
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            3
        );
        assert_eq!(
            profile_measurement_id(kind, "community", "facility"),
            format!("{}:community:facility", ids[0].split(':').next().unwrap())
        );
    }
}

#[test]
fn entity_ids_normalize_uuid_case_only() {
    let canonical = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
    for id in [canonical.to_string(), canonical.to_uppercase()] {
        assert_eq!(normalize_uuid(&id).unwrap(), canonical);
    }
}

#[test]
fn entity_parser_rejects_non_v4_and_alternate_notations() {
    for id in [
        "", "MARKET-1", "0x1234",
        "0x3f2504e04f8941d39a0c0305e82c3301",
        "3f2504e04f8941d39a0c0305e82c3301",
        "0X3f2504e04f8941d39a0c0305e82c3301",
        " 3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "3f2504e0-4f89-41d3-9a0c-0305e82c3301\n",
        "{3f2504e0-4f89-41d3-9a0c-0305e82c3301}",
        "urn:uuid:3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "0x3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "3f2504e0-4f89-11d3-9a0c-0305e82c3301",
        "3f2504e0-4f89-41d3-7a0c-0305e82c3301",
        "3f2504e04f89-41d3-9a0c-0305-e82c3301",
        "00000000-0000-0000-0000-000000000000",
    ] {
        assert!(normalize_uuid(id).is_err(), "{id:?}");
    }
}

#[test]
fn actor_hash_parser_accepts_only_prefixed_bytes16_hex() {
    assert_eq!(hex_to_bytes16("0xABABABABABABABABABABABABABABABAB"), Some([0xab; 16]));
    // Zero remains available for optional contract sentinels.
    assert_eq!(hex_to_bytes16("0x00000000000000000000000000000000"), Some([0; 16]));
    for id in ["abababababababababababababababab", "0Xabababababababababababababababab",
        "abababab-abab-4bab-8bab-abababababab", "0x1234",
        " 0xabababababababababababababababab", "0xabababababababababababababababab\n"] {
        assert_eq!(hex_to_bytes16(id), None, "{id:?}");
    }
}

#[test]
fn offchain_normalization_preserves_opaque_and_hex_shaped_identities() {
    let uuid = "ABABABAB-ABAB-ABAB-ABAB-ABABABABABAB";
    assert_eq!(normalize_offchain_id(uuid).unwrap(), uuid);
    for value in [
        "facility-a",
        "Facility-A",
        "0xABABABABABABABABABABABABABABABAB",
        "ABABABABABABABABABABABABABABABAB",
        "DS-1-MQTT",
    ] {
        assert_eq!(normalize_offchain_id(value).unwrap(), value);
    }
    for value in [
        "",
        " ",
        "\t\n",
        " facility-a",
        "facility-a\n",
        "facility-a\u{a0}",
    ] {
        assert!(normalize_offchain_id(value).is_err(), "{value:?}");
    }
}

#[test]
fn canonical_outputs_round_trip_in_their_identifier_domains() {
    let bytes = *uuid::Uuid::new_v4().as_bytes();
    let entity = bytes16_to_uuid_string(bytes);
    assert_eq!(uuid_v4_to_bytes16(&entity).unwrap(), bytes);
    assert_eq!(normalize_uuid(&normalize_uuid(&entity).unwrap()).unwrap(), entity);
    let actor_hash = derive_onchain_id("facility-a");
    assert_eq!(bytes16_to_hex(hex_to_bytes16(&actor_hash).unwrap()), actor_hash);
    assert!(uuid_v4_to_bytes16(&actor_hash).is_err());
    for actor in ["Facility-A", "ABABABAB-ABAB-4BAB-8BAB-ABABABABABAB", &actor_hash] {
        assert_eq!(normalize_offchain_id(&normalize_offchain_id(actor).unwrap()).unwrap(), actor);
    }
}
