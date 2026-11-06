use primitives::utils::{generate_market_id, normalize_community_id};
use primitives::MarketType;

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn market_id_is_deterministic_for_the_same_seed() {
    let first = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();
    let second = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000).unwrap();

    assert_eq!(first, second);
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
