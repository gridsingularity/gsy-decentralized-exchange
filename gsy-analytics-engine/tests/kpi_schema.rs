use mongodb::bson::{self, Bson};
use primitives::db_api_schema::kpi::{
    KpiNullReason, MarketTimeSeriesGranularity, ProcurementCostComponents,
    ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};

fn sample_result() -> ProcurementCostResultSchema {
    ProcurementCostResultSchema {
        kpi_id: PROCUREMENT_COST_PER_KWH_KPI_ID.to_string(),
        community_id: "Pilot1".to_string(),
        granularity: MarketTimeSeriesGranularity::FifteenMinutes,
        period_start: 1_758_621_600,
        period_end: 1_758_622_500,
        value: None,
        null_reason: Some(KpiNullReason::ZeroNetDemand),
        unit: "EUR/kWh".to_string(),
        components: ProcurementCostComponents {
            trade_count: 1,
            ..Default::default()
        },
        baseline: None,
        computed_at: 1_758_624_000,
        engine_version: "0.1.0".to_string(),
    }
}

#[test]
fn procurement_cost_result_maps_to_expected_bson_types() {
    let document = bson::to_document(&sample_result()).unwrap();

    assert_eq!(
        document.get("period_start"),
        Some(&Bson::Int64(1_758_621_600))
    );
    assert_eq!(
        document.get("period_end"),
        Some(&Bson::Int64(1_758_622_500))
    );
    assert_eq!(
        document.get("computed_at"),
        Some(&Bson::Int64(1_758_624_000))
    );
    assert_eq!(document.get_str("granularity").unwrap(), "15min");
    assert_eq!(document.get("value"), Some(&Bson::Null));
    assert_eq!(document.get_str("null_reason").unwrap(), "zero_net_demand");
    assert_eq!(document.get("baseline"), Some(&Bson::Null));
}

#[test]
fn procurement_cost_result_round_trips_through_bson() {
    let result = sample_result();
    let document = bson::to_document(&result).unwrap();

    assert_eq!(
        bson::from_document::<ProcurementCostResultSchema>(document).unwrap(),
        result
    );
}
