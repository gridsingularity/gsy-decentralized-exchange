use gsy_community_client::external_forecasts::pv_api::{pv_avg_watts_to_kwh, PvForecastPoint};
use gsy_community_client::external_forecasts::pv_pricing::{
    commitment_from_point, offer_commitment, PvCommitment, PvCommitmentConfig,
};

const TOL: f64 = 1e-9;

/// Build a config identical to the defaults but with an overridden risk aversion,
/// so the risk-aversion sweep does not depend on env state.
fn cfg_with_risk(risk_aversion: f64) -> PvCommitmentConfig {
    PvCommitmentConfig {
        risk_aversion,
        spread_norm: 1.0,
        min_confidence: 0.1,
        min_forecast_kwh: 0.05,
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < TOL, "expected {} ~= {}", a, b);
}

#[test]
fn offer_risk_aversion_one_commits_q5() {
    let c = offer_commitment(1.0, 0.6, 1.4, &cfg_with_risk(1.0));
    approx(c.energy_kwh, 0.6);
}

#[test]
fn offer_risk_aversion_zero_commits_point_forecast() {
    let c = offer_commitment(1.0, 0.6, 1.4, &cfg_with_risk(0.0));
    approx(c.energy_kwh, 1.0);
}

#[test]
fn offer_risk_aversion_half_commits_midpoint() {
    let c = offer_commitment(1.0, 0.6, 1.4, &cfg_with_risk(0.5));
    // 1.0 - 0.5 * (1.0 - 0.6) = 0.8
    approx(c.energy_kwh, 0.8);
}

#[test]
fn offer_clamps_q5_above_forecast_to_forecast() {
    // Misbehaving forecaster: q5 > forecast. Commitment must not exceed the forecast.
    let c = offer_commitment(1.0, 1.5, 2.0, &cfg_with_risk(1.0));
    approx(c.energy_kwh, 1.0);
    assert!(c.energy_kwh >= 0.0);
}

#[test]
fn offer_result_never_negative() {
    let c = offer_commitment(0.5, -0.3, 1.0, &cfg_with_risk(1.0));
    assert!(c.energy_kwh >= 0.0);
}

#[test]
fn offer_night_slot_zero_energy_confidence_one() {
    let c = offer_commitment(0.0, 0.0, 0.0, &PvCommitmentConfig::default());
    approx(c.energy_kwh, 0.0);
    approx(c.confidence, 1.0);
    assert!(c.energy_kwh.is_finite() && c.confidence.is_finite());
}

#[test]
fn offer_negative_forecast_treated_as_night() {
    let c = offer_commitment(-5.0, -6.0, -1.0, &PvCommitmentConfig::default());
    approx(c.energy_kwh, 0.0);
    approx(c.confidence, 1.0);
}

#[test]
fn confidence_narrow_band_near_one() {
    // Tiny spread relative to a healthy forecast => confidence ~ 1.
    let c = offer_commitment(2.0, 1.99, 2.01, &cfg_with_risk(1.0));
    assert!(c.confidence > 0.99, "confidence was {}", c.confidence);
    assert!(c.confidence <= 1.0);
}

#[test]
fn confidence_wide_band_clamped_to_min() {
    // Spread == forecast with spread_norm 1.0 => 1 - 1 = 0 => clamped to min_confidence.
    let cfg = cfg_with_risk(1.0);
    let c = offer_commitment(1.0, 0.5, 1.5, &cfg);
    approx(c.confidence, cfg.min_confidence);
    assert!(c.confidence >= cfg.min_confidence && c.confidence < 1.0);
}

#[test]
fn confidence_always_within_bounds() {
    let cfg = cfg_with_risk(1.0);
    for &(f, q5, q95) in &[
        (1.0, 0.9, 1.1),
        (1.0, 0.0, 5.0),
        (0.1, 0.05, 0.5),
        (3.0, 2.99, 3.0),
        (0.5, 0.4, 0.6),
    ] {
        let c = offer_commitment(f, q5, q95, &cfg);
        assert!(
            c.confidence >= cfg.min_confidence && c.confidence <= 1.0,
            "confidence {} out of range for ({}, {}, {})",
            c.confidence,
            f,
            q5,
            q95
        );
    }
}

#[test]
fn commitment_from_point_applies_same_kwh_factor_to_all_three() {
    // Real sample values from the PV forecaster.
    let point = PvForecastPoint {
        timestamp: chrono::NaiveDateTime::parse_from_str(
            "2026-07-15T04:30:00",
            "%Y-%m-%dT%H:%M:%S",
        )
        .unwrap(),
        pv_forecast: 123.68235294117646,
        p5: vec![61.84117647058823, 74.20941176470588],
        p95: vec![160.7870588235294, 166.9711764705882],
    };
    let cfg = PvCommitmentConfig {
        risk_aversion: 1.0,
        spread_norm: 1.0,
        min_confidence: 0.1,
        min_forecast_kwh: 0.05,
    };
    let c = commitment_from_point(&point, &cfg);
    // risk_aversion 1.0 => commit q5, which is the min of p5 array (61.841...W)
    // converted to kWh with the same factor as forecast and q95.
    approx(c.energy_kwh, pv_avg_watts_to_kwh(61.84117647058823));
    assert!(c.confidence >= cfg.min_confidence && c.confidence <= 1.0);
}

#[test]
fn commitment_struct_is_comparable() {
    let a = PvCommitment {
        energy_kwh: 1.0,
        confidence: 0.5,
    };
    let b = PvCommitment {
        energy_kwh: 1.0,
        confidence: 0.5,
    };
    assert_eq!(a, b);
}
