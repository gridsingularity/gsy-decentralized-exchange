use crate::constants::{CommunityClientConstants, INTER_COMMUNITY_MARKET_NAME};
use crate::external_measurements::metering_points::{MeteringPointKind, MeteringPointSet};
use crate::offchain_storage_connector::adapter::{deterministic_area_hash, generate_market_id};
use gsy_offchain_primitives::MarketType;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::h256_to_string;
use once_cell::sync::Lazy;
use std::collections::{BTreeSet, HashSet};
use subxt::utils::H256;

/// Communities allowed to participate in the inter-community market, read once from
/// `INTER_COMMUNITY_ELIGIBLE_COMMUNITIES`.
static ELIGIBLE_COMMUNITIES: Lazy<Vec<String>> = Lazy::new(|| {
    parse_community_list(&CommunityClientConstants.INTER_COMMUNITY_ELIGIBLE_COMMUNITIES)
});

/// Suffix of the SmartMeter asset `T + "SM"` of the Influx meter token `T`.
const SMART_METER_SUFFIX: &str = "SM";

/// Deterministic, community-independent id of the single inter-community market
/// for a delivery timeslot.
pub fn inter_community_market_id(time_slot: u64) -> H256 {
    generate_market_id(INTER_COMMUNITY_MARKET_NAME, MarketType::Spot, time_slot)
}

/// Parse a comma-separated list of community names. Names are trimmed, and empty entries
/// are skipped, so a blank spec gives an empty list.
pub fn parse_community_list(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// The communities allowed to participate in the inter-community market.
pub fn eligible_inter_communities() -> &'static [String] {
    &ELIGIBLE_COMMUNITIES
}

pub fn eligible_inter_community(community_name: &str) -> bool {
    ELIGIBLE_COMMUNITIES
        .iter()
        .any(|eligible| eligible == community_name)
}

/// How much of a community's demand is forecast for a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemandCoverage {
    /// Number of SmartMeters whose demand forecast is expected.
    pub expected: usize,
    /// Names of the expected SmartMeters without a positive forecast, sorted.
    pub missing: Vec<String>,
}

impl DemandCoverage {
    pub fn is_full(&self) -> bool {
        self.expected > 0 && self.missing.is_empty()
    }
}

/// The demand coverage of `community` in `slot_forecasts` (its forecasts for one slot).
///
/// A community's net order is only meaningful if its whole demand is forecast. The
/// expected SmartMeters are `T + "SM"` for every expected meter `T` of every building
/// point of `community` in `set`; one is covered if a forecast with its area hash in
/// `community` has a positive `energy_kwh`.
pub fn demand_coverage(
    set: &MeteringPointSet,
    community: &str,
    slot_forecasts: &[ForecastSchema],
) -> DemandCoverage {
    let forecast_hashes: HashSet<&str> = slot_forecasts
        .iter()
        .filter(|forecast| forecast.energy_kwh > 0.0)
        .map(|forecast| forecast.area_hash.as_str())
        .collect();
    let expected: BTreeSet<String> = set
        .points
        .iter()
        .filter(|point| {
            point.community_name == community && point.kind == MeteringPointKind::Building
        })
        .flat_map(|point| point.expected_meters.iter())
        .map(|token| format!("{token}{SMART_METER_SUFFIX}"))
        .collect();
    let missing = expected
        .iter()
        .filter(|asset| {
            let hash = h256_to_string(deterministic_area_hash(community, asset));
            !forecast_hashes.contains(hash.as_str())
        })
        .cloned()
        .collect();
    DemandCoverage {
        expected: expected.len(),
        missing,
    }
}
