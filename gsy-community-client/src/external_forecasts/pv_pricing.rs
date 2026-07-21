//! Percentile-based offer-energy commitment and confidence-modulated rate floor for
//! PV-forecast-driven orders.
//!
//! Two levers operate off the same PV forecast:
//! - Energy quantity: a point forecast plus its p5/p95 quantile bounds is converted
//!   into the single energy quantity committed to an on-chain offer (see
//!   [`offer_commitment`]).
//! - Offer rate floor (now active): the per-slot [`PvCommitment::confidence`] scalar
//!   raises the lower bound of the offer's price ramp when confidence is low (see
//!   [`effective_offer_min_rate`]).

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

/// Confidence-modulated lower bound (floor) for an offer's price ramp.
///
/// The offer's time ramp runs from `max_rate` down to this floor instead of down to
/// `min_rate`. Rationale for a seller: an unmatched offer carries no penalty, whereas
/// a matched-but-underproduced offer does. Low confidence therefore raises the floor
/// so that uncertain energy only matches at prices carrying a risk premium; high
/// confidence (`confidence -> 1.0`) leaves the floor at `min_rate`, i.e. today's
/// behavior.
///
/// `weight` scales the effect: `0.0` disables modulation entirely (floor stays at
/// `min_rate`); `1.0` lets a zero-confidence offer ramp no lower than `max_rate`.
/// The result is clamped to `[min_rate, max_rate]`; `confidence` outside `[0, 1]` is
/// clamped defensively.
pub fn effective_offer_min_rate(min_rate: f64, max_rate: f64, confidence: f64, weight: f64) -> f64 {
    let confidence = confidence.clamp(0.0, 1.0);
    let effective = min_rate + (1.0 - confidence) * weight * (max_rate - min_rate);
    effective.clamp(min_rate, max_rate)
}

/// Bridging the PV API forecast point type to an offer commitment.
pub fn commitment_from_point(point: &PvForecastPoint, cfg: &PvCommitmentConfig) -> PvCommitment {
    let (q5_watts, q95_watts) = point.quantile_bounds();
    let forecast_kwh = pv_avg_watts_to_kwh(point.pv_forecast);
    let q5_kwh = pv_avg_watts_to_kwh(q5_watts);
    let q95_kwh = pv_avg_watts_to_kwh(q95_watts);
    offer_commitment(forecast_kwh, q5_kwh, q95_kwh, cfg)
}
