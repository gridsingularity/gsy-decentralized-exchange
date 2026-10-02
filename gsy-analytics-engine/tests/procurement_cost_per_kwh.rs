use gsy_analytics_engine::config::TariffConfig;
use gsy_analytics_engine::kpi::procurement_cost_per_kwh::{
    baseline, compute_slot_components, kpi_value, ProcurementCostPerKwhKpi,
};
use gsy_analytics_engine::kpi::{Kpi, KpiContext, KpiResult};
use gsy_analytics_engine::model::{Dataset, MeterReading, TradeRecord, TradeStatus};
use gsy_analytics_engine::period::Window;
use primitives::db_api_schema::kpi::{
    KpiNullReason, MarketTimeSeriesGranularity, ProcurementCostResultSchema,
};
use std::collections::HashMap;
use std::sync::Arc;

const EPSILON: f64 = 1e-9;
const SLOT: i64 = 1_758_621_600; // 2025-09-23 10:00 UTC
const SLOT_LENGTH: i64 = 900;
const COMMUNITY: &str = "Pilot1";

fn trade(buyer: &str, energy_kwh: f64, energy_rate: f64) -> TradeRecord {
    TradeRecord {
        trade_uuid: format!("{}-{}-{}", buyer, energy_kwh, energy_rate),
        community_id: Some(COMMUNITY.to_string()),
        buyer: buyer.to_string(),
        time_slot: SLOT,
        energy_kwh,
        energy_rate,
        status: TradeStatus::Settled,
    }
}

fn reading(facility: &str, energy_kwh: f64) -> MeterReading {
    MeterReading {
        community_id: COMMUNITY.to_string(),
        facility: facility.to_string(),
        time_slot: SLOT,
        energy_kwh,
    }
}

/// The worked example from the plan (§3.2).
fn example_trades() -> Vec<TradeRecord> {
    vec![trade("A", 3.0, 0.12), trade("B", 1.5, 0.15)]
}

fn example_readings() -> Vec<MeterReading> {
    vec![reading("A", 4.0), reading("B", 2.0), reading("P", -5.0)]
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < EPSILON,
        "expected {}, got {}",
        expected,
        actual
    );
}

fn refs<T>(items: &[T]) -> Vec<&T> {
    items.iter().collect()
}

#[test]
fn worked_example_components() {
    let trades = example_trades();
    let readings = example_readings();
    let components = compute_slot_components(&refs(&trades), &refs(&readings), Some(0.30));

    assert_close(components.p2p_cost, 0.585);
    assert_close(components.p2p_energy_kwh, 4.5);
    assert_close(components.net_demand_kwh, 6.0);
    assert_close(components.grid_residual_energy_kwh, 1.5);
    assert_close(components.grid_residual_cost.unwrap(), 0.45);
    assert_close(components.community_net_import_kwh, 1.0);
    assert_close(components.baseline_cost.unwrap(), 1.8);
    assert_eq!(components.tariff_eur_per_kwh, Some(0.30));
    assert_eq!(components.trade_count, 2);
    assert_eq!(components.facility_count, 3);
    assert_eq!(components.facilities_with_reading, 3);
    assert_eq!(components.buyers_without_reading, 0);
}

#[test]
fn worked_example_value_and_baseline() {
    let trades = example_trades();
    let readings = example_readings();
    let components = compute_slot_components(&refs(&trades), &refs(&readings), Some(0.30));

    let value = kpi_value(&components).unwrap();
    assert_close(value, 0.1725);

    let baseline = baseline(&components, Some(value)).unwrap();
    assert_close(baseline.baseline_value.unwrap(), 0.30);
    assert_close(baseline.improvement_pct.unwrap(), 42.5);
}

#[test]
fn over_purchase_has_no_residual_and_keeps_full_p2p_cost() {
    let trades = vec![trade("A", 5.0, 0.10)];
    let readings = vec![reading("A", 3.0)];
    let components = compute_slot_components(&refs(&trades), &refs(&readings), Some(0.30));

    assert_close(components.grid_residual_energy_kwh, 0.0);
    assert_close(components.net_demand_kwh, 3.0);
    assert_close(kpi_value(&components).unwrap(), 0.5 / 3.0);
}

#[test]
fn buyer_without_reading_uses_traded_energy_as_demand() {
    let trades = vec![trade("A", 3.0, 0.12), trade("C", 2.0, 0.10)];
    let readings = vec![reading("A", 4.0)];
    let components = compute_slot_components(&refs(&trades), &refs(&readings), Some(0.30));

    assert_eq!(components.buyers_without_reading, 1);
    assert_eq!(components.facility_count, 2);
    assert_eq!(components.facilities_with_reading, 1);
    assert_close(components.net_demand_kwh, 6.0);
    assert_close(components.grid_residual_energy_kwh, 1.0);
}

#[test]
fn readings_without_trades_cost_the_tariff() {
    let readings = vec![reading("A", 4.0), reading("B", 1.0)];
    let components = compute_slot_components(&[], &refs(&readings), Some(0.30));

    let value = kpi_value(&components).unwrap();
    assert_close(value, 0.30);
    assert_close(
        baseline(&components, Some(value))
            .unwrap()
            .improvement_pct
            .unwrap(),
        0.0,
    );
}

#[test]
fn zero_demand_gives_null_value_and_no_baseline() {
    let readings = vec![reading("P", -5.0)];
    let components = compute_slot_components(&[], &refs(&readings), Some(0.30));

    assert_eq!(kpi_value(&components), Err(KpiNullReason::ZeroNetDemand));
    assert_eq!(baseline(&components, None), None);
}

#[test]
fn missing_tariff_gives_null_value_and_no_baseline() {
    let trades = example_trades();
    let readings = example_readings();
    let components = compute_slot_components(&refs(&trades), &refs(&readings), None);

    assert_eq!(components.grid_residual_cost, None);
    assert_eq!(components.baseline_cost, None);
    assert_close(components.p2p_cost, 0.585);
    assert_eq!(kpi_value(&components), Err(KpiNullReason::MissingTariff));
    assert_eq!(baseline(&components, None), None);
}

#[test]
fn rejected_trades_are_excluded() {
    let mut rejected = trade("A", 3.0, 0.12);
    rejected.status = TradeStatus::Rejected;
    let trades = vec![rejected];
    let readings = vec![reading("A", 4.0)];
    let components = compute_slot_components(&refs(&trades), &refs(&readings), Some(0.30));

    assert_eq!(components.trade_count, 0);
    assert_close(components.p2p_cost, 0.0);
    assert_close(components.grid_residual_energy_kwh, 4.0);
}

#[test]
fn duplicate_readings_for_a_facility_are_summed() {
    let readings = vec![reading("A", 3.0), reading("A", 1.0), reading("A", -0.5)];
    let components = compute_slot_components(&[], &refs(&readings), Some(0.30));

    assert_eq!(components.facilities_with_reading, 1);
    assert_close(components.net_demand_kwh, 3.5);
    assert_close(components.community_net_import_kwh, 3.5);
}

fn run_kpi(dataset: &Dataset, tariffs: TariffConfig) -> Vec<ProcurementCostResultSchema> {
    let window = Window {
        start: SLOT - SLOT_LENGTH,
        end: SLOT + 2 * SLOT_LENGTH,
        slot_length: SLOT_LENGTH,
    };
    let kpi = ProcurementCostPerKwhKpi::new(Arc::new(tariffs));
    let context = KpiContext {
        window: &window,
        dataset,
        computed_at: 1_758_624_000,
    };
    kpi.compute(&context)
        .into_iter()
        .map(|result| match result {
            KpiResult::ProcurementCostPerKwh(result) => result,
        })
        .collect()
}

fn flat_tariff(tariff: f64) -> TariffConfig {
    TariffConfig {
        default_eur_per_kwh: Some(tariff),
        overrides: HashMap::new(),
    }
}

#[test]
fn compute_builds_one_result_per_community_and_slot() {
    let mut trades = example_trades();
    let mut late = trade("A", 1.0, 0.20);
    late.time_slot = SLOT + SLOT_LENGTH + 120; // unaligned, floors to the next slot
    trades.push(late);
    let mut other_community = reading("X", 2.0);
    other_community.community_id = "Pilot2".to_string();
    let mut readings = example_readings();
    readings.push(other_community);
    let dataset = Dataset {
        trades,
        readings,
        ..Default::default()
    };

    let results = run_kpi(&dataset, flat_tariff(0.30));
    let keys: Vec<(&str, i64)> = results
        .iter()
        .map(|result| (result.community_id.as_str(), result.period_start))
        .collect();
    assert_eq!(
        keys,
        vec![
            ("Pilot1", SLOT),
            ("Pilot1", SLOT + SLOT_LENGTH),
            ("Pilot2", SLOT)
        ]
    );

    let example = &results[0];
    assert_eq!(example.kpi_id, "procurement_cost_per_kwh");
    assert_eq!(
        example.granularity,
        MarketTimeSeriesGranularity::FifteenMinutes
    );
    assert_eq!(example.period_end, SLOT + SLOT_LENGTH);
    assert_eq!(example.unit, "EUR/kWh");
    assert_close(example.value.unwrap(), 0.1725);
    assert_eq!(example.null_reason, None);
    assert_close(
        example.baseline.as_ref().unwrap().improvement_pct.unwrap(),
        42.5,
    );
    assert_eq!(example.computed_at, 1_758_624_000);

    // Buyer with no reading in the later slot: demand = bought, no residual.
    assert_close(results[1].value.unwrap(), 0.20);
    assert_eq!(results[1].components.buyers_without_reading, 1);
}

#[test]
fn compute_skips_unassigned_trades_and_slots_outside_the_window() {
    let mut unassigned = trade("A", 3.0, 0.12);
    unassigned.community_id = None;
    let mut too_old = trade("A", 3.0, 0.12);
    too_old.time_slot = SLOT - 2 * SLOT_LENGTH;
    let mut too_new = reading("A", 4.0);
    too_new.time_slot = SLOT + 2 * SLOT_LENGTH;
    let dataset = Dataset {
        trades: vec![unassigned, too_old],
        readings: vec![too_new],
        ..Default::default()
    };

    assert!(run_kpi(&dataset, flat_tariff(0.30)).is_empty());
}

#[test]
fn compute_uses_per_community_tariff_overrides() {
    let dataset = Dataset {
        readings: vec![reading("A", 2.0)],
        ..Default::default()
    };
    let tariffs = TariffConfig {
        default_eur_per_kwh: Some(0.30),
        overrides: HashMap::from([(COMMUNITY.to_string(), 0.25)]),
    };

    let results = run_kpi(&dataset, tariffs);
    assert_eq!(results[0].components.tariff_eur_per_kwh, Some(0.25));
    assert_close(results[0].value.unwrap(), 0.25);
}

#[test]
fn compute_records_null_reason_without_tariff() {
    let dataset = Dataset {
        trades: example_trades(),
        readings: example_readings(),
        ..Default::default()
    };
    let tariffs = TariffConfig {
        default_eur_per_kwh: None,
        overrides: HashMap::new(),
    };

    let results = run_kpi(&dataset, tariffs);
    assert_eq!(results[0].value, None);
    assert_eq!(results[0].null_reason, Some(KpiNullReason::MissingTariff));
    assert_eq!(results[0].baseline, None);
}
