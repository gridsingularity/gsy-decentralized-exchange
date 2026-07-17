//! Percentile-based offer-energy commitment for PV-forecast-driven orders.
//! Converts a point forecast plus its p5/p95 quantile bounds into
//! the single energy quantity that is committed to an on-chain offer, together with
//! a per-slot confidence scalar.
//! Only the *energy quantity* of an offer is calculated using the percentiles;
//! energy rates are not modified. The [`PvCommitment::confidence`] scalar is
//! still computed and returned so that energy rates can be adapted.

use crate::constants::CommunityClientConstants;
use crate::external_forecasts::pv_api::{pv_avg_watts_to_kwh, PvForecastPoint};

/// Configuration for PV energy commitment.
#[derive(Debug, Clone, Copy)]
pub struct PvCommitmentConfig {
    /// 0.0 to commit the forecasted energy, 1.0 to commit the conservative percentile.
    pub risk_aversion: f64,
    /// Normalizer for the relative spread when deriving confidence.
    pub spread_norm: f64,
    /// Lower clamp for the confidence scalar.
    pub min_confidence: f64,
    /// Floor (kWh) for the denominator of the relative spread. Avoids div-by-zero
    /// at night or near-zero output.
    pub min_forecast_kwh: f64,
}

impl PvCommitmentConfig {
    /// Build config from constants.rs.
    pub fn from_constants() -> Self {
        Self {
            risk_aversion: CommunityClientConstants.PV_RISK_AVERSION,
            spread_norm: CommunityClientConstants.PV_SPREAD_NORM,
            min_confidence: CommunityClientConstants.PV_MIN_CONFIDENCE,
            min_forecast_kwh: CommunityClientConstants.PV_MIN_FORECAST_KWH,
        }
    }
}

impl Default for PvCommitmentConfig {
    /// Same values as [`PvCommitmentConfig::from_constants`].
    fn default() -> Self {
        Self::from_constants()
    }
}

/// The commitment result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PvCommitment {
    /// Energy to commit in the order, kWh (>= 0; 0 means "do not post an order").
    pub energy_kwh: f64,
    /// Per-slot forecast confidence in `[min_confidence, 1.0]`.
    pub confidence: f64,
}

/// Confidence scalar derived from the relative width of the p5..p95 band.
fn confidence_from_spread(forecast_kwh: f64, q5_kwh: f64, q95_kwh: f64, cfg: &PvCommitmentConfig) -> f64 {
    let relative_spread = (q95_kwh - q5_kwh) / forecast_kwh.max(cfg.min_forecast_kwh);
    (1.0 - relative_spread / cfg.spread_norm).clamp(cfg.min_confidence, 1.0)
}

/// Production / offer-side commitment (kWh in, kWh out).
pub fn offer_commitment(
    forecast_kwh: f64,
    q5_kwh: f64,
    q95_kwh: f64,
    cfg: &PvCommitmentConfig,
) -> PvCommitment {
    if forecast_kwh <= 0.0 {
        return PvCommitment {
            energy_kwh: 0.0,
            confidence: 1.0,
        };
    }

    // Lower and upper bounds of the valid commitment window. Guard against a
    // misbehaving forecaster reporting q5 above the point forecast.
    let lower = q5_kwh.min(forecast_kwh);
    let energy_kwh = (forecast_kwh - cfg.risk_aversion * (forecast_kwh - lower))
        .clamp(lower, forecast_kwh)
        .max(0.0);

    PvCommitment {
        energy_kwh,
        confidence: confidence_from_spread(forecast_kwh, q5_kwh, q95_kwh, cfg),
    }
}

/// Bridging the PV API forecast point type to an offer commitment.
pub fn commitment_from_point(point: &PvForecastPoint, cfg: &PvCommitmentConfig) -> PvCommitment {
    let (q5_watts, q95_watts) = point.quantile_bounds();
    let forecast_kwh = pv_avg_watts_to_kwh(point.pv_forecast);
    let q5_kwh = pv_avg_watts_to_kwh(q5_watts);
    let q95_kwh = pv_avg_watts_to_kwh(q95_watts);
    offer_commitment(forecast_kwh, q5_kwh, q95_kwh, cfg)
}
