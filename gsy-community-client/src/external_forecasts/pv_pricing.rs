//! Percentile-based energy commitment for forecast-driven orders.
//!
//! A point forecast plus its p5/p95 quantile bounds is converted into the single energy
//! quantity committed to an on-chain order (see [`commitment`]). A signed risk factor
//! `s` in `[-1, +1]` expresses *risk appetite*, with one uniform meaning on both sides
//! of the market: `-1` is maximally conservative, `0` is the point forecast and `+1` is
//! maximally optimistic. Which tail of the band counts as conservative differs per side
//! (a seller commits toward q5, a buyer toward q95); that is handled internally by
//! [`PvCommitmentConfig::side_sign`], not by the sign of `s`.
//!
//! The per-slot [`PvCommitment::confidence`] scalar is still derived from the band width
//! and persisted on `ForecastSchema`, but it no longer influences order pricing.

use crate::constants::CommunityClientConstants;
use crate::external_forecasts::pv_api::{pv_avg_watts_to_kwh, PvForecastPoint};
use tracing::warn;

/// Configuration for percentile-based energy commitment.
#[derive(Debug, Clone, Copy)]
pub struct PvCommitmentConfig {
    /// Signed risk appetite in `[-1.0, +1.0]`, with the same meaning on both sides of
    /// the market:
    ///
    /// * `-1.0` — maximally conservative: an offer commits approximately q5, a bid
    ///   approximately q95.
    /// * `0.0` — neutral: exactly the point forecast, on both sides.
    /// * `+1.0` — maximally optimistic: an offer commits approximately q95, a bid
    ///   approximately q5.
    ///
    /// The per-side difference in *which* tail is the conservative one is encoded by
    /// [`Self::side_sign`], never by the sign of this field. Values outside the range are
    /// clamped on use.
    ///
    /// NOT in standard-deviation units: the applied displacement is
    /// `side_sign * s * (q95 - q5) / 2`, and under normality
    /// `(q95 - q5) / 2 ~= 1.645 * sigma`.
    pub risk_factor: f64,
    /// Which direction inside the band the conservative end lies in for this market
    /// side: `+1.0` for offers, `-1.0` for bids.
    ///
    /// NOT user-configurable — it is not read from the environment and has no default to
    /// tune. It exists solely so that a single `risk_factor` scale can mean the same
    /// thing to a seller and to a buyer: the penalty model
    /// (`gsy-execution-engine` `penalty_calculator`) is one-sided per role — a seller is
    /// only penalised when measured production falls *below* the energy sold, a buyer
    /// only when measured consumption rises *above* the energy bought — so a buyer's
    /// conservative direction is the opposite tail to a seller's.
    ///
    /// Set by [`Self::for_offers`] / [`Self::for_demand`]; do not set it by hand.
    pub side_sign: f64,
    /// Normalizer for the relative spread when deriving confidence.
    pub spread_norm: f64,
    /// Lower clamp for the confidence scalar.
    pub min_confidence: f64,
    /// Floor (kWh) for the denominator of the relative spread. Avoids div-by-zero
    /// at night or near-zero output.
    pub min_forecast_kwh: f64,
}

impl PvCommitmentConfig {
    /// Offer/production-side config: `risk_factor` from `OFFER_RISK_FACTOR`
    /// (default `-1.0`, i.e. maximally conservative) and `side_sign = +1.0`, so a
    /// conservative offer commits toward the lower q5 tail.
    pub fn for_offers() -> Self {
        Self {
            risk_factor: CommunityClientConstants.OFFER_RISK_FACTOR,
            side_sign: 1.0,
            spread_norm: CommunityClientConstants.PV_SPREAD_NORM,
            min_confidence: CommunityClientConstants.PV_MIN_CONFIDENCE,
            min_forecast_kwh: CommunityClientConstants.PV_MIN_FORECAST_KWH,
        }
    }

    /// Demand/bid-side config: the same spread/confidence tuning as the offer side (the
    /// confidence derivation is side-agnostic), with `risk_factor` from `BID_RISK_FACTOR`
    /// (default `-1.0`, i.e. maximally conservative) and `side_sign = -1.0`, so a
    /// conservative bid commits toward the upper q95 tail.
    pub fn for_demand() -> Self {
        Self {
            risk_factor: CommunityClientConstants.BID_RISK_FACTOR,
            side_sign: -1.0,
            ..Self::for_offers()
        }
    }
}

impl Default for PvCommitmentConfig {
    /// Same values as [`PvCommitmentConfig::for_offers`].
    fn default() -> Self {
        Self::for_offers()
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

/// Percentile-based energy commitment (kWh in, kWh out):
///
/// ```text
/// E = max(0, F + side_sign * s * (q95 - q5) / 2)
/// ```
///
/// where `s` is [`PvCommitmentConfig::risk_factor`], clamped to `[-1.0, +1.0]`, and
/// `side_sign` is [`PvCommitmentConfig::side_sign`] (`+1` for offers, `-1` for bids).
///
/// `s` therefore has one uniform meaning on both sides of the market: `-1` is maximally
/// conservative (offer commits ~q5, bid commits ~q95), `0` is the point forecast on both
/// sides, `+1` is maximally optimistic (offer ~q95, bid ~q5). The per-side flip lives in
/// `side_sign` because the penalty model (`gsy-execution-engine` `penalty_calculator`) is
/// one-sided per role: a seller is only penalised when measured production falls *below*
/// the energy sold, a buyer only when measured consumption rises *above* the energy
/// bought.
///
/// The half-band displacement only reproduces the quantile exactly for a symmetric band;
/// for a skewed one it is an approximation. A wide band with a conservative `s` can drive
/// an offer below zero, in which case it clamps to `0.0` and the caller posts no order.
///
/// A non-positive point forecast is treated as an inactive slot: zero energy, full
/// confidence.
pub fn commitment(
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

    // Data-quality signals only: the formula below is applied unchanged in both cases,
    // nothing is clamped to the band.
    if q95_kwh < q5_kwh {
        warn!(
            "Forecaster reported an inverted quantile interval: q95 ({}) below q5 ({}); \
             committing on the inverted band as-is (no clamping applied)",
            q95_kwh, q5_kwh
        );
    }
    if forecast_kwh < q5_kwh || forecast_kwh > q95_kwh {
        warn!(
            "Miscalibrated forecaster: point forecast ({}) lies outside its own \
             q5..q95 band ({}..{}); committing on the reported band as-is \
             (no clamping applied)",
            forecast_kwh, q5_kwh, q95_kwh
        );
    }

    let s = cfg.risk_factor.clamp(-1.0, 1.0);
    let half_band = (q95_kwh - q5_kwh) / 2.0;
    let energy_kwh = (forecast_kwh + cfg.side_sign * s * half_band).max(0.0);

    PvCommitment {
        energy_kwh,
        confidence: confidence_from_spread(forecast_kwh, q5_kwh, q95_kwh, cfg),
    }
}

/// Bridging the PV API forecast point type to a commitment.
pub fn commitment_from_point(point: &PvForecastPoint, cfg: &PvCommitmentConfig) -> PvCommitment {
    let (q5_watts, q95_watts) = point.quantile_bounds();
    let forecast_kwh = pv_avg_watts_to_kwh(point.pv_forecast);
    let q5_kwh = pv_avg_watts_to_kwh(q5_watts);
    let q95_kwh = pv_avg_watts_to_kwh(q95_watts);
    commitment(forecast_kwh, q5_kwh, q95_kwh, cfg)
}
