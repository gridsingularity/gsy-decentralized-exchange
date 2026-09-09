use primitives::utils::generate_market_id;
use primitives::MarketType;

const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn market_id_is_deterministic_for_the_same_seed() {
    let first = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);
    let second = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);

    assert_eq!(first, second);
}

#[test]
fn market_id_changes_for_each_seed_component() {
    let market_id = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);

    assert_ne!(
        market_id,
        generate_market_id(
            "22222222-2222-4222-8222-222222222222",
            MarketType::Spot,
            1_700_000_000,
        )
    );
    assert_ne!(
        market_id,
        generate_market_id(COMMUNITY_ID, MarketType::Flex, 1_700_000_000)
    );
    assert_ne!(
        market_id,
        generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_001)
    );
}
