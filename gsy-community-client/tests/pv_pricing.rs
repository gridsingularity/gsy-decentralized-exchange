use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::pv_api::{pv_avg_watts_to_kwh, PvForecastPoint};
use gsy_community_client::external_forecasts::pv_pricing::{
    commitment, commitment_from_point, PvCommitment, PvCommitmentConfig,
};

const TOL: f64 = 1e-9;

/// Offer-side config (`side_sign = +1.0`) identical to the shipped defaults but with an
/// overridden risk factor, so the risk-factor sweep does not depend on env state.
fn cfg_with_risk(risk_factor: f64) -> PvCommitmentConfig {
    cfg_for_side(risk_factor, 1.0)
}

/// Bid-side config (`side_sign = -1.0`), same tuning otherwise.
fn cfg_bid(risk_factor: f64) -> PvCommitmentConfig {
    cfg_for_side(risk_factor, -1.0)
}

fn cfg_for_side(risk_factor: f64, side_sign: f64) -> PvCommitmentConfig {
    PvCommitmentConfig {
        risk_factor,
        side_sign,
        spread_norm: 1.0,
        min_confidence: 0.1,
        min_forecast_kwh: 0.05,
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < TOL, "expected {} ~= {}", a, b);
}

// ---- E = max(0, F + side_sign * s * (q95 - q5) / 2) -----------------------
//
// This block pins the OFFER side (side_sign = +1); the bid side is covered further down.

#[test]
fn zero_risk_factor_commits_exactly_the_point_forecast() {
    let c = commitment(1.0, 0.6, 1.4, &cfg_with_risk(0.0));
    approx(c.energy_kwh, 1.0);
}

#[test]
fn minus_one_commits_forecast_minus_half_band_which_is_q5_for_a_symmetric_band() {
    // half_band = (1.4 - 0.6) / 2 = 0.4; 1.0 - 0.4 = 0.6 == q5 (band symmetric about F).
    let c = commitment(1.0, 0.6, 1.4, &cfg_with_risk(-1.0));
    approx(c.energy_kwh, 0.6);
}

#[test]
fn plus_one_commits_forecast_plus_half_band_which_is_q95_for_a_symmetric_band() {
    // 1.0 + 0.4 = 1.4 == q95.
    let c = commitment(1.0, 0.6, 1.4, &cfg_with_risk(1.0));
    approx(c.energy_kwh, 1.4);
}

#[test]
fn risk_factor_is_linear_between_the_endpoints() {
    // s = -0.5 => the quarter point: 1.0 - 0.5 * 0.4 = 0.8.
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(-0.5)).energy_kwh, 0.8);
    // s = +0.5, still the offer side: 1.0 + 0.5 * 0.4 = 1.2.
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(0.5)).energy_kwh, 1.2);
}

#[test]
fn skewed_band_half_band_only_approximates_the_quantile() {
    // Skewed band: q5 = 0.8, q95 = 2.0, F = 1.0 => half_band = 0.6.
    // s = -1 gives 1.0 - 0.6 = 0.4, which is NOT q5 (0.8): documents the approximation.
    let c = commitment(1.0, 0.8, 2.0, &cfg_with_risk(-1.0));
    approx(c.energy_kwh, 0.4);
    assert!(
        (c.energy_kwh - 0.8).abs() > TOL,
        "s = -1 is a half-band displacement, not the exact q5, for a skewed band"
    );
    // Same on the upper side: 1.0 + 0.6 = 1.6 != q95 (2.0).
    let c = commitment(1.0, 0.8, 2.0, &cfg_with_risk(1.0));
    approx(c.energy_kwh, 1.6);
    assert!((c.energy_kwh - 2.0).abs() > TOL);
}

#[test]
fn wide_band_with_negative_risk_factor_clamps_to_zero_and_posts_no_order() {
    // Behavioural change: q5 = 0.2 > 0, yet half_band = (3.0 - 0.2) / 2 = 1.4 exceeds
    // F = 1.0, so the commitment goes negative and clamps to 0 -> caller posts no order.
    let c = commitment(1.0, 0.2, 3.0, &cfg_with_risk(-1.0));
    approx(c.energy_kwh, 0.0);
    assert!(c.energy_kwh.is_finite() && c.confidence.is_finite());
    assert!(c.confidence >= 0.1 && c.confidence <= 1.0);
}

#[test]
fn result_is_never_negative() {
    for &(f, q5, q95, s) in &[
        (0.5, -0.3, 1.0, -1.0),
        (0.1, 0.0, 10.0, -1.0),
        (2.0, 1.0, 100.0, -1.0),
        (1.0, 5.0, 6.0, -1.0),
    ] {
        let c = commitment(f, q5, q95, &cfg_with_risk(s));
        assert!(c.energy_kwh >= 0.0, "negative energy for ({f}, {q5}, {q95}, s={s})");
    }
}

// ---- inactive slots -------------------------------------------------------

#[test]
fn zero_forecast_is_an_inactive_slot() {
    let c = commitment(0.0, 0.0, 0.0, &PvCommitmentConfig::default());
    approx(c.energy_kwh, 0.0);
    approx(c.confidence, 1.0);
    assert!(c.energy_kwh.is_finite() && c.confidence.is_finite());
}

#[test]
fn negative_forecast_is_an_inactive_slot_for_every_risk_factor() {
    for &s in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
        for &f in &[0.0, -0.001, -5.0] {
            let c = commitment(f, f - 1.0, f + 1.0, &cfg_with_risk(s));
            approx(c.energy_kwh, 0.0);
            approx(c.confidence, 1.0);
        }
    }
}

// ---- defensive clamping of s ---------------------------------------------

#[test]
fn risk_factor_below_minus_one_is_clamped() {
    approx(
        commitment(1.0, 0.6, 1.4, &cfg_with_risk(-7.0)).energy_kwh,
        commitment(1.0, 0.6, 1.4, &cfg_with_risk(-1.0)).energy_kwh,
    );
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(-7.0)).energy_kwh, 0.6);
}

#[test]
fn risk_factor_above_plus_one_is_clamped() {
    approx(
        commitment(1.0, 0.6, 1.4, &cfg_with_risk(9.5)).energy_kwh,
        commitment(1.0, 0.6, 1.4, &cfg_with_risk(1.0)).energy_kwh,
    );
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(9.5)).energy_kwh, 1.4);
}

// ---- malformed forecasts still compute (no clamping to the band) ---------

#[test]
fn point_forecast_outside_its_own_band_is_not_clamped() {
    // q5 (1.5) above F (1.0): warns, but the formula still applies.
    // half_band = (2.0 - 1.5) / 2 = 0.25 => 1.0 - 0.25 = 0.75.
    approx(commitment(1.0, 1.5, 2.0, &cfg_with_risk(-1.0)).energy_kwh, 0.75);
    // q95 (0.5) below F (1.0): half_band = (0.5 - 0.2) / 2 = 0.15 => 1.0 + 0.15 = 1.15.
    approx(commitment(1.0, 0.2, 0.5, &cfg_with_risk(1.0)).energy_kwh, 1.15);
}

#[test]
fn inverted_interval_does_not_panic_and_flips_the_displacement() {
    // q95 < q5 => half_band negative; s = -1 therefore raises the commitment.
    let c = commitment(1.0, 1.4, 0.6, &cfg_with_risk(-1.0));
    approx(c.energy_kwh, 1.4);
    assert!(c.energy_kwh.is_finite() && c.confidence.is_finite());
}

// ---- confidence: band-only, independent of s ------------------------------

#[test]
fn confidence_narrow_band_near_one() {
    let c = commitment(2.0, 1.99, 2.01, &cfg_with_risk(-1.0));
    assert!(c.confidence > 0.99, "confidence was {}", c.confidence);
    assert!(c.confidence <= 1.0);
}

#[test]
fn confidence_wide_band_clamped_to_min() {
    // Spread == forecast with spread_norm 1.0 => 1 - 1 = 0 => clamped to min_confidence.
    let cfg = cfg_with_risk(-1.0);
    let c = commitment(1.0, 0.5, 1.5, &cfg);
    approx(c.confidence, cfg.min_confidence);
    assert!(c.confidence >= cfg.min_confidence && c.confidence < 1.0);
}

#[test]
fn confidence_is_independent_of_the_risk_factor() {
    for &(f, q5, q95) in &[
        (1.0, 0.6, 1.4),
        (1.0, 0.0, 5.0),
        (0.1, 0.05, 0.5),
        (3.0, 2.99, 3.0),
        (0.5, 0.4, 0.6),
    ] {
        let reference = commitment(f, q5, q95, &cfg_with_risk(0.0)).confidence;
        for &s in &[-1.0, -0.5, 0.5, 1.0] {
            let cfg = cfg_with_risk(s);
            let c = commitment(f, q5, q95, &cfg);
            approx(c.confidence, reference);
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
}

// ---- config wiring --------------------------------------------------------

#[test]
fn offer_and_demand_configs_differ_only_in_the_risk_factor_and_side_sign() {
    let offers = PvCommitmentConfig::for_offers();
    let demand = PvCommitmentConfig::for_demand();
    approx(offers.spread_norm, demand.spread_norm);
    approx(offers.min_confidence, demand.min_confidence);
    approx(offers.min_forecast_kwh, demand.min_forecast_kwh);
    // Default is the offer-side config.
    approx(PvCommitmentConfig::default().risk_factor, offers.risk_factor);
    approx(PvCommitmentConfig::default().side_sign, offers.side_sign);
}

#[test]
fn side_sign_is_pinned_per_side_and_not_folded_into_the_risk_factor() {
    let offers = PvCommitmentConfig::for_offers();
    let demand = PvCommitmentConfig::for_demand();
    approx(offers.side_sign, 1.0);
    approx(demand.side_sign, -1.0);
    // `risk_factor` must stay the raw user-configured constant, never sign-flipped at
    // construction time, so the struct field matches the env var it came from.
    approx(offers.risk_factor, CommunityClientConstants.OFFER_RISK_FACTOR);
    approx(demand.risk_factor, CommunityClientConstants.BID_RISK_FACTOR);
}

#[test]
fn shipped_defaults_are_conservative_on_both_sides() {
    // Both sides now read "maximally conservative" on the same scale.
    approx(CommunityClientConstants.OFFER_RISK_FACTOR, -1.0);
    approx(CommunityClientConstants.BID_RISK_FACTOR, -1.0);
}

// ---- PV point bridging ----------------------------------------------------

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
    let cfg = cfg_with_risk(-1.0);
    let c = commitment_from_point(&point, &cfg);
    // The bounds are min(p5) and max(p95), converted with the same W -> kWh factor as
    // the point forecast, so the whole formula can be evaluated in kWh space.
    let f = pv_avg_watts_to_kwh(123.68235294117646);
    let q5 = pv_avg_watts_to_kwh(61.84117647058823);
    let q95 = pv_avg_watts_to_kwh(166.9711764705882);
    approx(c.energy_kwh, (f - (q95 - q5) / 2.0).max(0.0));
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

// ---- uniform risk-appetite scale across both market sides -----------------
//
// The whole point of `side_sign`: one `s` value means the same thing to a seller and to
// a buyer, even though the conservative tail is the opposite one on each side.

#[test]
fn s_minus_one_is_conservative_on_both_sides() {
    // Symmetric band about F = 1.0: half_band = 0.4, so q5 = 0.6 and q95 = 1.4.
    let offer = commitment(1.0, 0.6, 1.4, &cfg_with_risk(-1.0));
    let bid = commitment(1.0, 0.6, 1.4, &cfg_bid(-1.0));
    // Offer: sell only what you will likely produce -> F - half_band ~= q5.
    approx(offer.energy_kwh, 0.6);
    // Bid: buy enough to cover likely consumption -> F + half_band ~= q95.
    approx(bid.energy_kwh, 1.4);
    // Same s, opposite displacement.
    approx(offer.energy_kwh, 1.0 - 0.4);
    approx(bid.energy_kwh, 1.0 + 0.4);
}

#[test]
fn s_plus_one_is_optimistic_on_both_sides() {
    // Mirror image of the conservative case.
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(1.0)).energy_kwh, 1.4); // offer ~ q95
    approx(commitment(1.0, 0.6, 1.4, &cfg_bid(1.0)).energy_kwh, 0.6); // bid ~ q5
}

#[test]
fn s_zero_is_the_point_forecast_on_both_sides() {
    approx(commitment(1.0, 0.6, 1.4, &cfg_with_risk(0.0)).energy_kwh, 1.0);
    approx(commitment(1.0, 0.6, 1.4, &cfg_bid(0.0)).energy_kwh, 1.0);
    // Also for a skewed band: with s = 0 the band is irrelevant.
    approx(commitment(1.0, 0.8, 2.0, &cfg_with_risk(0.0)).energy_kwh, 1.0);
    approx(commitment(1.0, 0.8, 2.0, &cfg_bid(0.0)).energy_kwh, 1.0);
}

#[test]
fn the_two_sides_are_exact_mirrors_for_any_s() {
    // offer(s) - F == -(bid(s) - F) for every s. Bands are chosen with
    // half_band < F so neither side hits the max(0, ..) clamp, which is the only thing
    // that can break the mirror (see wide_band_clamp_to_zero_still_holds_on_the_offer_side).
    for &(f, q5, q95) in &[(1.0, 0.6, 1.4), (2.0, 1.0, 3.0), (0.5, 0.45, 0.55)] {
        for &s in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
            let offer = commitment(f, q5, q95, &cfg_with_risk(s)).energy_kwh;
            let bid = commitment(f, q5, q95, &cfg_bid(s)).energy_kwh;
            approx(offer - f, -(bid - f));
        }
    }
}

// ---- equivalence regression against the previous defaults -----------------
//
// Old semantics: `s` was a position inside the band and there was no `side_sign`, so
// OFFER_RISK_FACTOR = -1.0 and BID_RISK_FACTOR = +1.0 both meant "conservative". The new
// defaults (both -1.0, with side_sign doing the flip) must produce identical energies.

/// Old formula: `max(0, F + s * (q95 - q5) / 2)`, no side sign.
fn legacy_commitment(f: f64, q5: f64, q95: f64, s: f64) -> f64 {
    if f <= 0.0 {
        return 0.0;
    }
    (f + s.clamp(-1.0, 1.0) * (q95 - q5) / 2.0).max(0.0)
}

#[test]
fn new_defaults_reproduce_the_old_on_chain_energies_symmetric_band() {
    let (f, q5, q95) = (1.0, 0.6, 1.4);
    // Offers: new default -1.0 vs old default -1.0.
    approx(
        commitment(f, q5, q95, &cfg_with_risk(-1.0)).energy_kwh,
        legacy_commitment(f, q5, q95, -1.0),
    );
    // Bids: new default -1.0 (side_sign = -1) vs old default +1.0 (no side sign).
    approx(
        commitment(f, q5, q95, &cfg_bid(-1.0)).energy_kwh,
        legacy_commitment(f, q5, q95, 1.0),
    );
    // And the literal on-chain values: offers commit ~q5, bids ~q95.
    approx(commitment(f, q5, q95, &cfg_with_risk(-1.0)).energy_kwh, 0.6);
    approx(commitment(f, q5, q95, &cfg_bid(-1.0)).energy_kwh, 1.4);
}

#[test]
fn new_defaults_reproduce_the_old_on_chain_energies_asymmetric_band() {
    // Skewed band: half_band = (2.0 - 0.8) / 2 = 0.6.
    let (f, q5, q95) = (1.0, 0.8, 2.0);
    approx(
        commitment(f, q5, q95, &cfg_with_risk(-1.0)).energy_kwh,
        legacy_commitment(f, q5, q95, -1.0),
    );
    approx(
        commitment(f, q5, q95, &cfg_bid(-1.0)).energy_kwh,
        legacy_commitment(f, q5, q95, 1.0),
    );
    approx(commitment(f, q5, q95, &cfg_with_risk(-1.0)).energy_kwh, 0.4);
    approx(commitment(f, q5, q95, &cfg_bid(-1.0)).energy_kwh, 1.6);
}

#[test]
fn new_defaults_reproduce_the_old_energies_over_a_sweep_of_bands() {
    for &(f, q5, q95) in &[
        (1.0, 0.6, 1.4),
        (1.0, 0.8, 2.0),
        (1.0, 0.2, 3.0), // wide band: the offer side clamps to zero on both formulations
        (2.5, 2.4, 2.6),
        (0.1, 0.0, 0.5),
        (0.0, 0.0, 0.0), // inactive slot
        (-1.0, -2.0, 0.0),
    ] {
        approx(
            commitment(f, q5, q95, &cfg_with_risk(-1.0)).energy_kwh,
            legacy_commitment(f, q5, q95, -1.0),
        );
        approx(
            commitment(f, q5, q95, &cfg_bid(-1.0)).energy_kwh,
            legacy_commitment(f, q5, q95, 1.0),
        );
    }
}

// ---- side_sign does not leak into confidence or the zero clamp ------------

#[test]
fn confidence_is_independent_of_the_side_sign() {
    for &(f, q5, q95) in &[(1.0, 0.6, 1.4), (1.0, 0.0, 5.0), (0.1, 0.05, 0.5)] {
        for &s in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
            approx(
                commitment(f, q5, q95, &cfg_with_risk(s)).confidence,
                commitment(f, q5, q95, &cfg_bid(s)).confidence,
            );
        }
    }
}

#[test]
fn wide_band_clamp_to_zero_still_holds_on_the_offer_side() {
    // half_band = (3.0 - 0.2) / 2 = 1.4 > F = 1.0 => conservative offer clamps to 0.
    let c = commitment(1.0, 0.2, 3.0, &cfg_with_risk(-1.0));
    approx(c.energy_kwh, 0.0);
    // The mirrored bid does NOT clamp: it commits F + half_band.
    approx(commitment(1.0, 0.2, 3.0, &cfg_bid(-1.0)).energy_kwh, 2.4);
    // The bid clamps instead when it is optimistic.
    approx(commitment(1.0, 0.2, 3.0, &cfg_bid(1.0)).energy_kwh, 0.0);
}

#[test]
fn bid_side_result_is_never_negative() {
    for &(f, q5, q95, s) in &[
        (0.5, -0.3, 1.0, 1.0),
        (0.1, 0.0, 10.0, 1.0),
        (2.0, 1.0, 100.0, 1.0),
        (1.0, 5.0, 6.0, 1.0),
    ] {
        let c = commitment(f, q5, q95, &cfg_bid(s));
        assert!(c.energy_kwh >= 0.0, "negative energy for ({f}, {q5}, {q95}, s={s})");
    }
}
