//! KPI result schemas.
//!
//! `KpiResultSchema` is one KPI value for one community and one period, written by the
//! analytics engine to the `kpi_results` collection. It is unique per
//! `(kpi_id, community_id, granularity, period_start)`. `components` holds the KPI-specific
//! summable inputs, so that the value can be recomputed or rolled up later.

pub use crate::MarketTimeSeriesGranularity;
use serde::{Deserialize, Serialize};

pub const PROCUREMENT_COST_PER_KWH_KPI_ID: &str = "procurement_cost_per_kwh";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KpiResultSchema<C> {
    pub kpi_id: String,
    pub community_id: String,
    pub granularity: MarketTimeSeriesGranularity,
    /// Unix seconds, UTC.
    pub period_start: i64,
    /// Unix seconds, UTC.
    pub period_end: i64,
    pub value: Option<f64>,
    /// Why `value` is null. `None` when `value` is set.
    pub null_reason: Option<KpiNullReason>,
    pub unit: String,
    pub components: C,
    /// Base-case comparison. `None` when no base case could be computed.
    pub baseline: Option<KpiBaseline>,
    /// Unix seconds, UTC.
    pub computed_at: i64,
    pub engine_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KpiNullReason {
    ZeroNetDemand,
    MissingTariff,
    NoData,
}

/// Comparison of a KPI value against its base case (e.g. no P2P trading).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct KpiBaseline {
    pub baseline_value: Option<f64>,
    /// `(baseline_value - value) / baseline_value * 100`.
    pub improvement_pct: Option<f64>,
}

/// Summable inputs of the procurement cost per kWh KPI for one community and period.
/// Energies are in kWh, costs in EUR.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ProcurementCostComponents {
    pub p2p_cost: f64,
    pub p2p_energy_kwh: f64,
    pub net_demand_kwh: f64,
    pub grid_residual_energy_kwh: f64,
    /// `None` when no tariff is configured for the community.
    pub grid_residual_cost: Option<f64>,
    pub community_net_import_kwh: f64,
    /// Grid tariff in EUR/kWh used for the residual and baseline costs.
    pub tariff_eur_per_kwh: Option<f64>,
    /// Cost of buying the whole net demand from the grid.
    pub baseline_cost: Option<f64>,
    pub trade_count: u32,
    pub facility_count: u32,
    pub facilities_with_reading: u32,
    pub buyers_without_reading: u32,
}

pub type ProcurementCostResultSchema = KpiResultSchema<ProcurementCostComponents>;
