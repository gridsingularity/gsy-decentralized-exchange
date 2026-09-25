//! Procurement cost per kWh: the average cost a community pays for each kWh it consumes,
//! with P2P trading in place.
//!
//! `value = (P2P cost + residual grid energy × tariff) / member net demand`, per community and
//! 15-minute delivery slot. The base case buys the whole net demand from the grid.

use super::{DataRequirements, Kpi, KpiContext, KpiResult, ENGINE_VERSION};
use crate::model::{MeterReading, TradeRecord, TradeStatus};
use crate::period::align_to_slot;
use crate::tariff::TariffProvider;
use primitives::db_api_schema::kpi::{
    KpiBaseline, KpiNullReason, MarketTimeSeriesGranularity, ProcurementCostComponents,
    ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

pub const UNIT: &str = "EUR/kWh";

/// Sums the components for one community and slot. Rejected trades are ignored.
/// A buyer without a meter reading is assumed to have consumed exactly what it bought.
pub fn compute_slot_components(
    trades: &[&TradeRecord],
    readings: &[&MeterReading],
    tariff: Option<f64>,
) -> ProcurementCostComponents {
    let mut components = ProcurementCostComponents::default();

    let mut bought: HashMap<&str, f64> = HashMap::new();
    for trade in trades
        .iter()
        .filter(|trade| trade.status != TradeStatus::Rejected)
    {
        components.p2p_cost += trade.energy_kwh * trade.energy_rate;
        components.p2p_energy_kwh += trade.energy_kwh;
        components.trade_count += 1;
        *bought.entry(trade.buyer.as_str()).or_default() += trade.energy_kwh;
    }

    let mut net: HashMap<&str, f64> = HashMap::new();
    for reading in readings {
        *net.entry(reading.facility.as_str()).or_default() += reading.energy_kwh;
    }

    let facilities: BTreeSet<&str> = bought.keys().chain(net.keys()).copied().collect();
    for facility in &facilities {
        let bought_kwh = bought.get(facility).copied().unwrap_or(0.0);
        let demand_kwh = match net.get(facility) {
            Some(net_kwh) => net_kwh.max(0.0),
            None => {
                components.buyers_without_reading += 1;
                bought_kwh
            }
        };
        components.net_demand_kwh += demand_kwh;
        components.grid_residual_energy_kwh += (demand_kwh - bought_kwh).max(0.0);
    }

    components.community_net_import_kwh = net.values().sum();
    components.facility_count = facilities.len() as u32;
    components.facilities_with_reading = net.len() as u32;
    components.tariff_eur_per_kwh = tariff;
    components.grid_residual_cost = tariff.map(|t| components.grid_residual_energy_kwh * t);
    components.baseline_cost = tariff.map(|t| components.net_demand_kwh * t);
    components
}

pub fn kpi_value(components: &ProcurementCostComponents) -> Result<f64, KpiNullReason> {
    if components.net_demand_kwh <= 0.0 {
        return Err(KpiNullReason::ZeroNetDemand);
    }
    let grid_cost = components
        .grid_residual_cost
        .ok_or(KpiNullReason::MissingTariff)?;
    Ok((components.p2p_cost + grid_cost) / components.net_demand_kwh)
}

/// Comparison with buying the whole net demand from the grid. `None` without a tariff or demand.
pub fn baseline(components: &ProcurementCostComponents, value: Option<f64>) -> Option<KpiBaseline> {
    if components.net_demand_kwh <= 0.0 {
        return None;
    }
    let baseline_value = components.baseline_cost? / components.net_demand_kwh;
    let improvement_pct = match value {
        Some(value) if baseline_value > 0.0 => {
            Some((baseline_value - value) / baseline_value * 100.0)
        }
        _ => None,
    };
    Some(KpiBaseline {
        baseline_value: Some(baseline_value),
        improvement_pct,
    })
}

pub struct ProcurementCostPerKwhKpi {
    tariffs: Arc<dyn TariffProvider>,
}

impl ProcurementCostPerKwhKpi {
    pub fn new(tariffs: Arc<dyn TariffProvider>) -> Self {
        ProcurementCostPerKwhKpi { tariffs }
    }
}

type SlotData<'a> = (Vec<&'a TradeRecord>, Vec<&'a MeterReading>);

impl Kpi for ProcurementCostPerKwhKpi {
    fn id(&self) -> &'static str {
        PROCUREMENT_COST_PER_KWH_KPI_ID
    }

    fn unit(&self) -> &'static str {
        UNIT
    }

    fn requirements(&self) -> DataRequirements {
        DataRequirements {
            communities: true,
            trades: true,
            readings: true,
        }
    }

    /// One result per community and slot in the window that has trades or readings.
    fn compute(&self, context: &KpiContext) -> Vec<KpiResult> {
        let window = context.window;
        let mut slots: BTreeMap<(&str, i64), SlotData> = BTreeMap::new();

        for trade in &context.dataset.trades {
            let Some(community_id) = trade.community_id.as_deref() else {
                continue;
            };
            let slot = align_to_slot(trade.time_slot, window.slot_length);
            if window.contains_slot(slot) {
                slots.entry((community_id, slot)).or_default().0.push(trade);
            }
        }
        for reading in &context.dataset.readings {
            let slot = align_to_slot(reading.time_slot, window.slot_length);
            if window.contains_slot(slot) {
                slots
                    .entry((reading.community_id.as_str(), slot))
                    .or_default()
                    .1
                    .push(reading);
            }
        }

        slots
            .into_iter()
            .map(|((community_id, slot), (trades, readings))| {
                let tariff = self.tariffs.tariff_for(community_id, slot);
                let components = compute_slot_components(&trades, &readings, tariff);
                let (value, null_reason) = match kpi_value(&components) {
                    Ok(value) => (Some(value), None),
                    Err(reason) => (None, Some(reason)),
                };
                KpiResult::ProcurementCostPerKwh(ProcurementCostResultSchema {
                    kpi_id: PROCUREMENT_COST_PER_KWH_KPI_ID.to_string(),
                    community_id: community_id.to_string(),
                    granularity: MarketTimeSeriesGranularity::FifteenMinutes,
                    period_start: slot,
                    period_end: slot + window.slot_length,
                    value,
                    null_reason,
                    unit: UNIT.to_string(),
                    baseline: baseline(&components, value),
                    components,
                    computed_at: context.computed_at,
                    engine_version: ENGINE_VERSION.to_string(),
                })
            })
            .collect()
    }
}
