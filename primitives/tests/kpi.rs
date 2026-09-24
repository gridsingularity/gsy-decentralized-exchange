use primitives::db_api_schema::kpi::{
    KpiBaseline, KpiNullReason, MarketTimeSeriesGranularity, ProcurementCostComponents,
    ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};
use serde_json::json;

fn sample_result() -> ProcurementCostResultSchema {
    ProcurementCostResultSchema {
        kpi_id: PROCUREMENT_COST_PER_KWH_KPI_ID.to_string(),
        community_id: "Pilot1".to_string(),
        granularity: MarketTimeSeriesGranularity::FifteenMinutes,
        period_start: 1_758_621_600,
        period_end: 1_758_622_500,
        value: Some(0.1725),
        null_reason: None,
        unit: "EUR/kWh".to_string(),
        components: ProcurementCostComponents {
            p2p_cost: 0.585,
            p2p_energy_kwh: 4.5,
            net_demand_kwh: 6.0,
            grid_residual_energy_kwh: 1.5,
            grid_residual_cost: Some(0.45),
            community_net_import_kwh: 1.0,
            tariff_eur_per_kwh: Some(0.30),
            baseline_cost: Some(1.8),
            trade_count: 2,
            facility_count: 3,
            facilities_with_reading: 3,
            buyers_without_reading: 0,
        },
        baseline: Some(KpiBaseline {
            baseline_value: Some(0.30),
            improvement_pct: Some(42.5),
        }),
        computed_at: 1_758_624_000,
        engine_version: "0.1.0".to_string(),
    }
}

#[test]
fn procurement_cost_result_serializes_to_the_documented_shape() {
    let value = serde_json::to_value(sample_result()).unwrap();

    assert_eq!(
        value,
        json!({
            "kpi_id": "procurement_cost_per_kwh",
            "community_id": "Pilot1",
            "granularity": "15min",
            "period_start": 1_758_621_600,
            "period_end": 1_758_622_500,
            "value": 0.1725,
            "null_reason": null,
            "unit": "EUR/kWh",
            "components": {
                "p2p_cost": 0.585,
                "p2p_energy_kwh": 4.5,
                "net_demand_kwh": 6.0,
                "grid_residual_energy_kwh": 1.5,
                "grid_residual_cost": 0.45,
                "community_net_import_kwh": 1.0,
                "tariff_eur_per_kwh": 0.30,
                "baseline_cost": 1.8,
                "trade_count": 2,
                "facility_count": 3,
                "facilities_with_reading": 3,
                "buyers_without_reading": 0
            },
            "baseline": {
                "baseline_value": 0.30,
                "improvement_pct": 42.5
            },
            "computed_at": 1_758_624_000,
            "engine_version": "0.1.0"
        })
    );
}

#[test]
fn procurement_cost_result_round_trips() {
    let result = sample_result();
    let json = serde_json::to_string(&result).unwrap();

    assert_eq!(
        serde_json::from_str::<ProcurementCostResultSchema>(&json).unwrap(),
        result
    );
}

#[test]
fn null_value_carries_a_reason_and_no_baseline() {
    let result = ProcurementCostResultSchema {
        value: None,
        null_reason: Some(KpiNullReason::MissingTariff),
        baseline: None,
        ..sample_result()
    };
    let value = serde_json::to_value(&result).unwrap();

    assert_eq!(value["value"], json!(null));
    assert_eq!(value["null_reason"], json!("missing_tariff"));
    assert_eq!(value["baseline"], json!(null));
    assert_eq!(
        serde_json::from_value::<ProcurementCostResultSchema>(value).unwrap(),
        result
    );
}

#[test]
fn null_reasons_use_snake_case() {
    for (reason, expected) in [
        (KpiNullReason::ZeroNetDemand, "zero_net_demand"),
        (KpiNullReason::MissingTariff, "missing_tariff"),
        (KpiNullReason::NoData, "no_data"),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(expected));
    }
}
