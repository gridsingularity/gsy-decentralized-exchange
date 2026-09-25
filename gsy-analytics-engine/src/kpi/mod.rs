pub mod procurement_cost_per_kwh;

use crate::config::Config;
use crate::model::Dataset;
use crate::period::Window;
use anyhow::{bail, Result};
use primitives::db_api_schema::kpi::{
    MarketTimeSeriesGranularity, ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};
use procurement_cost_per_kwh::ProcurementCostPerKwhKpi;
use std::ops::BitOr;
use std::sync::Arc;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The data a KPI needs loaded for a tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataRequirements {
    pub communities: bool,
    pub trades: bool,
    pub readings: bool,
}

impl BitOr for DataRequirements {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        DataRequirements {
            communities: self.communities || other.communities,
            trades: self.trades || other.trades,
            readings: self.readings || other.readings,
        }
    }
}

pub struct KpiContext<'a> {
    pub window: &'a Window,
    pub dataset: &'a Dataset,
    /// Unix seconds.
    pub computed_at: i64,
}

/// One computed KPI document, ready to be upserted.
#[derive(Debug, Clone, PartialEq)]
pub enum KpiResult {
    ProcurementCostPerKwh(ProcurementCostResultSchema),
}

impl KpiResult {
    /// Fields of the unique `kpi_results` key.
    pub fn key(&self) -> (&str, &str, &MarketTimeSeriesGranularity, i64) {
        match self {
            KpiResult::ProcurementCostPerKwh(result) => (
                &result.kpi_id,
                &result.community_id,
                &result.granularity,
                result.period_start,
            ),
        }
    }
}

pub trait Kpi: Send + Sync {
    fn id(&self) -> &'static str;
    fn unit(&self) -> &'static str;
    fn requirements(&self) -> DataRequirements;
    /// Pure: no I/O.
    fn compute(&self, context: &KpiContext) -> Vec<KpiResult>;
}

pub const KNOWN_KPIS: &[&str] = &[PROCUREMENT_COST_PER_KWH_KPI_ID];

/// Builds the KPIs listed in `ANALYTICS_ENABLED_KPIS`. Fails on unknown or duplicate ids.
pub fn build_registry(config: &Config) -> Result<Vec<Box<dyn Kpi>>> {
    let mut kpis: Vec<Box<dyn Kpi>> = Vec::new();
    for id in &config.enabled_kpis {
        if kpis.iter().any(|kpi| kpi.id() == id) {
            bail!(
                "KPI '{}' is listed more than once in ANALYTICS_ENABLED_KPIS",
                id
            );
        }
        match id.as_str() {
            PROCUREMENT_COST_PER_KWH_KPI_ID => kpis.push(Box::new(ProcurementCostPerKwhKpi::new(
                Arc::new(config.tariffs.clone()),
            ))),
            other => bail!(
                "Unknown KPI '{}' in ANALYTICS_ENABLED_KPIS. Known KPIs: {}",
                other,
                KNOWN_KPIS.join(", ")
            ),
        }
    }
    if kpis.is_empty() {
        bail!("ANALYTICS_ENABLED_KPIS must list at least one KPI");
    }
    Ok(kpis)
}

/// Union of the data every KPI needs.
pub fn combined_requirements(kpis: &[Box<dyn Kpi>]) -> DataRequirements {
    kpis.iter().fold(DataRequirements::default(), |acc, kpi| {
        acc | kpi.requirements()
    })
}
