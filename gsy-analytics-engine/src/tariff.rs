use crate::config::TariffConfig;

/// Source of the grid tariff (EUR/kWh) a community pays for energy not bought P2P.
/// Configured from env vars for now; a database-backed source can replace it later.
pub trait TariffProvider: Send + Sync {
    fn tariff_for(&self, community_id: &str, slot_start: i64) -> Option<f64>;
}

impl TariffProvider for TariffConfig {
    fn tariff_for(&self, community_id: &str, _slot_start: i64) -> Option<f64> {
        self.overrides
            .get(community_id)
            .copied()
            .or(self.default_eur_per_kwh)
    }
}
