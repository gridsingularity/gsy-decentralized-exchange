use primitives::utils::{
    bytes16_to_hex, bytes16_to_uuid_string, canonical_offchain_id, derive_onchain_id,
    generate_market_id, normalize_community_id, normalize_market_id, normalize_offchain_id,
    parse_nonzero_uuid_or_hex_bytes16, parse_uuid_or_hex_bytes16,
};
use primitives::MarketType;

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn identity_derivation_canonicalizes_uuids_but_preserves_opaque_ids() {
    let lower = "abcdefab-1234-1234-1234-abcdefabcdef";
    assert_eq!(
        derive_onchain_id(&lower.to_uppercase()),
        derive_onchain_id(lower)
    );
    assert_eq!(
        derive_onchain_id(lower),
        bytes16_to_hex(primitives::utils::create_encrypted_bytes16_from_string(
            lower
        ))
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
        assert_eq!(
            primitives::utils::profile_measurement_id(kind.clone(), "community", lower),
            primitives::utils::profile_measurement_id(kind, "community", &lower.to_uppercase())
        );
    }
}

#[test]
fn profile_ids_canonicalize_uuid_community_references() {
    use primitives::db_api_schema::profiles::MeasurementPointType;
    use primitives::utils::profile_measurement_id;
    let community = "abcdefab-1234-1234-1234-abcdefabcdef";
    for kind in [
        MeasurementPointType::Measurement,
        MeasurementPointType::Forecast,
    ] {
        assert_eq!(
            profile_measurement_id(kind.clone(), community, "facility-a"),
            profile_measurement_id(kind, &community.to_uppercase(), "facility-a")
        );
    }
}

#[test]
fn onchain_identity_derivation_is_pinned_to_blake2b_16() {
    assert_eq!(
        primitives::utils::derive_onchain_id("facility-a"),
        "0x6bcb5f7af93a900ac5c3f27259bccc63"
    );
    assert_ne!(
        primitives::utils::derive_onchain_id("facility-a"),
        primitives::utils::derive_onchain_id("Facility-a")
    );
}

#[test]
fn market_id_normalizes_uuid_and_hex_forms_to_lowercase_hex() {
    let canonical = "0x3f2504e04f8941d39a0c0305e82c3301";
    for id in [
        canonical,
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301",
        "0x3F2504E04F8941D39A0C0305E82C3301",
        "3F2504E04F8941D39A0C0305E82C3301",
    ] {
        assert_eq!(normalize_market_id(id).unwrap(), canonical);
    }
}

#[test]
fn market_id_normalization_rejects_invalid_values() {
    for id in [
        "",
        "MARKET-1",
        "0x1234",
        "0xgggggggggggggggggggggggggggggggg",
        "0x1111111111111111111111111111111111111111111111111111111111111111",
    ] {
        assert!(normalize_market_id(id).is_err(), "{id}");
    }
}

#[test]
fn market_id_is_deterministic_for_the_same_seed() {
    let first = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();
    let second = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();

    assert_eq!(first, second);
    assert_eq!(bytes16_to_hex(first), "0x808f0603acbe66964d3da1cd0cb3f5f4");
}

#[test]
fn bytes16_parser_accepts_only_the_explicit_grammar() {
    let expected = [0xab; 16];
    for value in [
        "abababab-abab-abab-abab-abababababab",
        "ABABABAB-ABAB-ABAB-ABAB-ABABABABABAB",
        "0xABABABABABABABABABABABABABABABAB",
        "abababababababababababababababab",
    ] {
        assert_eq!(parse_uuid_or_hex_bytes16(value), Some(expected), "{value}");
    }
    for value in [
        "",
        "0xab",
        "0xgggggggggggggggggggggggggggggggg",
        "0Xabababababababababababababababab",
        " abababab-abab-abab-abab-abababababab",
        "abababab-abab-abab-abab-abababababab\n",
        "\tabababababababababababababababab",
        "{abababab-abab-abab-abab-abababababab}",
        "urn:uuid:abababab-abab-abab-abab-abababababab",
        "0xabababab-abab-abab-abab-abababababab",
        "abababababab-abab-abab-abab-abababab",
        "abababababababababababababababab----",
        "\u{e9}abababababababababababababababa",
    ] {
        assert_eq!(parse_uuid_or_hex_bytes16(value), None, "{value:?}");
    }
}

#[test]
fn required_ids_reject_zero_without_breaking_optional_sentinels() {
    for value in [
        "00000000-0000-0000-0000-000000000000",
        "0x00000000000000000000000000000000",
        "00000000000000000000000000000000",
    ] {
        assert_eq!(parse_uuid_or_hex_bytes16(value), Some([0; 16]));
        assert_eq!(parse_nonzero_uuid_or_hex_bytes16(value), None);
        assert!(normalize_community_id(value).is_err());
        assert!(normalize_market_id(value).is_err());
        assert!(generate_market_id(value, MarketType::Spot, 1_700_000_000).is_err());
    }
}

#[test]
fn offchain_normalization_preserves_opaque_and_hex_shaped_identities() {
    let uuid = "ABABABAB-ABAB-ABAB-ABAB-ABABABABABAB";
    assert_eq!(
        normalize_offchain_id(uuid).unwrap(),
        uuid.to_ascii_lowercase()
    );
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
        assert_eq!(canonical_offchain_id(value), value);
    }
}

#[test]
fn canonical_outputs_round_trip_in_their_identifier_domains() {
    let market = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();
    let facility = parse_uuid_or_hex_bytes16(&derive_onchain_id("facility-a")).unwrap();
    for bytes in [
        market,
        facility,
        [0xff; 16],
        *uuid::Uuid::new_v4().as_bytes(),
    ] {
        for value in [bytes16_to_hex(bytes), bytes16_to_uuid_string(bytes)] {
            assert_eq!(parse_nonzero_uuid_or_hex_bytes16(&value), Some(bytes));
            let market = normalize_market_id(&value).unwrap();
            assert_eq!(normalize_market_id(&market).unwrap(), market);
            let community = normalize_community_id(&value).unwrap();
            assert_eq!(normalize_community_id(&community).unwrap(), community);
            let offchain = normalize_offchain_id(&value).unwrap();
            assert_eq!(normalize_offchain_id(&offchain).unwrap(), offchain);
        }
    }
}

#[test]
fn market_id_changes_for_each_seed_component() {
    let market_id = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();

    assert_ne!(
        market_id,
        generate_market_id(
            "22222222-2222-4222-8222-222222222222",
            MarketType::Spot,
            1_700_000_000,
        )
        .unwrap()
    );
    assert_ne!(
        market_id,
        generate_market_id(COMMUNITY_ID, MarketType::Flex, 1_700_000_000).unwrap()
    );
    assert_ne!(
        market_id,
        generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_001).unwrap()
    );
}

#[test]
fn community_id_normalizes_uuid_and_hex_forms_to_lowercase_uuid() {
    let canonical = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
    for community_id in [
        canonical,
        "3F2504E0-4F89-41D3-9A0C-0305E82C3301",
        "0x3f2504e04f8941d39a0c0305e82c3301",
        "3F2504E04F8941D39A0C0305E82C3301",
    ] {
        assert_eq!(
            normalize_community_id(community_id).unwrap(),
            canonical,
            "{community_id}"
        );
    }
}

#[test]
fn community_id_rejects_non_uuid_ids() {
    assert!(normalize_community_id("community1").is_err());
}

#[test]
fn market_id_is_the_same_for_every_form_of_a_community_id() {
    let market_id = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();

    assert_eq!(
        market_id,
        generate_market_id(
            "0x11111111111141118111111111111111",
            MarketType::Spot,
            1_700_000_000,
        )
        .unwrap()
    );
}

#[test]
fn market_id_rejects_a_non_uuid_community_id() {
    assert!(generate_market_id("community1", MarketType::Spot, 1_700_000_000).is_err());
}
