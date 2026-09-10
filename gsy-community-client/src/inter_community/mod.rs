use crate::constants::INTER_COMMUNITY_MARKET_NAME;
use crate::offchain_storage_connector::adapter::generate_market_id;
use gsy_offchain_primitives::MarketType;
use subxt::utils::H256;

/// Communities allowed to participate in the inter-community market.
pub const INTER_COMMUNITY_ELIGIBLE_COMMUNITIES: [&str; 2] =
    ["LugaggiaInnovationCommunity", "GaramèDistrict"];

/// Deterministic, community-independent id of the single inter-community market
/// for a delivery timeslot.
pub fn inter_community_market_id(time_slot: u64) -> H256 {
    generate_market_id(INTER_COMMUNITY_MARKET_NAME, MarketType::Spot, time_slot)
}

pub fn eligible_inter_community(community_name: &str) -> bool {
    INTER_COMMUNITY_ELIGIBLE_COMMUNITIES.contains(&community_name)
}
