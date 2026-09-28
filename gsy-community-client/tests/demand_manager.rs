use chrono::{DateTime, NaiveDate, Utc};
use gsy_community_client::external_forecasts::demand_api::DemandForecastPoint;
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_forecasts::pv_pricing::PvCommitmentConfig;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, AssetType};

/// Bid-side config held explicitly so the mapping assertions do not depend on env
/// state. `side_sign` is `-1.0`, the bid-side value set by
/// `PvCommitmentConfig::for_demand`: on this side a conservative (`s = -1`) buyer
/// commits toward the UPPER q95 tail.
fn cfg_with_risk(risk_factor: f64) -> PvCommitmentConfig {
    PvCommitmentConfig {
        risk_factor,
        side_sign: -1.0,
        spread_norm: 1.0,
        min_confidence: 0.1,
        min_forecast_kwh: 0.05,
    }
}

fn meter_area() -> AreaTopologySchema {
    AreaTopologySchema {
        area_uuid: "meter-uuid".to_string(),
        name: "LIC08SM".to_string(),
        area_type: AssetType::SMART_METER,
        area_hash: "meter-hash".to_string(),
    }
}

fn slot_time() -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(2026, 7, 15)
        .unwrap()
        .and_hms_opt(4, 30, 0)
        .unwrap()
        .and_utc()
}

fn point(forecast: f64, p5: f64, p95: f64) -> DemandForecastPoint {
    DemandForecastPoint {
        timestamp: slot_time(),
        forecast,
        p5,
        p95,
    }
}

#[test]
fn mapping_produces_positive_energy_bid_at_q95() {
    // risk_factor -1.0 (maximally conservative, the same value a conservative seller
    // uses) => on the bid side the buyer commits the UPPER half-band, which for this
    // symmetric band is exactly p95.
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(-1.0),
    )
    .expect("a positive demand forecast should produce a bid");

    assert!((schema.energy_kwh - 1.4).abs() < 1e-9);
    // Positive energy marks a consumption bid.
    assert!(schema.energy_kwh > 0.0);
    assert_eq!(schema.area_uuid, "meter-uuid");
    assert_eq!(schema.area_hash, "meter-hash");
    assert_eq!(schema.community_uuid, "community-uuid");
}

#[test]
fn mapping_carries_real_per_slot_confidence_not_the_old_constant() {
    // relative_spread = (1.4 - 0.6) / 1.0 = 0.8 => confidence = 1 - 0.8 = 0.2.
    let p = point(1.0, 0.6, 1.4);
    let cfg = cfg_with_risk(-1.0);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg,
    )
    .unwrap();
    assert!((schema.confidence - 0.2).abs() < 1e-9);
    // Not the removed fixed DEMAND_FORECAST_CONFIDENCE of 0.9.
    assert_ne!(schema.confidence, 0.9);
    assert!(schema.confidence >= cfg.min_confidence && schema.confidence <= 1.0);
}

#[test]
fn mapping_time_slot_is_unix_seconds_of_utc_timestamp() {
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(-1.0),
    )
    .unwrap();
    assert_eq!(schema.time_slot, slot_time().timestamp() as u64);
}

#[test]
fn zero_demand_slot_is_skipped() {
    let p = point(0.0, 0.0, 0.0);
    assert!(
        ForecastsManager::demand_forecast_schema_from_point(
            &p,
            &meter_area(),
            "community-uuid",
            &cfg_with_risk(-1.0),
        )
        .is_none()
    );
}

#[test]
fn negative_demand_slot_is_skipped() {
    let p = point(-0.5, -1.0, -0.1);
    assert!(
        ForecastsManager::demand_forecast_schema_from_point(
            &p,
            &meter_area(),
            "community-uuid",
            &cfg_with_risk(-1.0),
        )
        .is_none()
    );
}

#[test]
fn zero_risk_factor_maps_the_point_forecast() {
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(0.0),
    )
    .unwrap();
    assert!((schema.energy_kwh - 1.0).abs() < 1e-9);
}

#[test]
fn malformed_q95_below_forecast_is_not_clamped_to_the_point_forecast() {
    // The point forecast lies outside its own band: the mapping warns but still applies
    // the half-band displacement. Bid side with s = -1 => 1.0 + (0.5 - 0.2) / 2 = 1.15.
    let p = point(1.0, 0.2, 0.5);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(-1.0),
    )
    .unwrap();
    assert!((schema.energy_kwh - 1.15).abs() < 1e-9);
    assert!(schema.energy_kwh > 0.0);
}

#[test]
fn optimistic_risk_factor_still_maps_to_positive_consumption_energy() {
    // The demand path is sign-fixed: whatever `s` is configured, a bid carries POSITIVE
    // energy. On the bid side (side_sign = -1) the optimistic s = +1 commits toward the
    // LOWER q5 tail: 1.0 - (1.4 - 0.6) / 2 = 0.6.
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(1.0),
    )
    .unwrap();
    assert!((schema.energy_kwh - 0.6).abs() < 1e-9);
    assert!(schema.energy_kwh > 0.0);
}

#[test]
fn wide_band_with_optimistic_risk_factor_commits_nothing_and_skips_the_slot() {
    // Bid side, optimistic s = +1 => the displacement is downward:
    // half_band = (3.0 - 0.2) / 2 = 1.4 > F = 1.0 => commitment clamps to 0 => no bid.
    let p = point(1.0, 0.2, 3.0);
    assert!(
        ForecastsManager::demand_forecast_schema_from_point(
            &p,
            &meter_area(),
            "community-uuid",
            &cfg_with_risk(1.0),
        )
        .is_none()
    );
}

#[test]
fn conservative_bid_and_conservative_offer_share_the_same_risk_factor() {
    // The point of the uniform scale: s = -1 is "maximally conservative" on BOTH sides.
    // On the bid side that means committing the upper q95 tail (buy enough), while the
    // offer side commits q5 -- see tests/pv_pricing.rs for the paired assertion.
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &cfg_with_risk(-1.0),
    )
    .unwrap();
    assert!((schema.energy_kwh - 1.4).abs() < 1e-9);
    assert!(schema.energy_kwh > 0.0, "a bid always carries positive energy");
}

#[test]
fn for_demand_pins_the_bid_side_sign_and_still_yields_a_q95_bid() {
    // `for_demand()` owns the side sign (not user-configurable); the risk factor is
    // pinned here so the assertion does not depend on env state.
    let cfg = PvCommitmentConfig::for_demand();
    assert!(
        (cfg.side_sign - (-1.0)).abs() < 1e-9,
        "for_demand must set side_sign = -1.0, got {}",
        cfg.side_sign
    );
    let conservative = PvCommitmentConfig {
        risk_factor: -1.0,
        ..cfg
    };
    let p = point(1.0, 0.6, 1.4);
    let schema = ForecastsManager::demand_forecast_schema_from_point(
        &p,
        &meter_area(),
        "community-uuid",
        &conservative,
    )
    .unwrap();
    assert!((schema.energy_kwh - 1.4).abs() < 1e-9);
    assert!(schema.energy_kwh > 0.0);
}
