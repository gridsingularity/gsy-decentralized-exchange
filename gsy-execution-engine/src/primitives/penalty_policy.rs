use anyhow::{ensure, Context, Result};
use primitives::utils::NODE_FLOAT_SCALING_FACTOR;

#[derive(Clone, Copy, Debug)]
pub struct PenaltyPolicy {
    min_rate: f64,
    max_rate: f64,
    tolerance: u128,
}

impl PenaltyPolicy {
    pub fn new(min_rate: f64, max_rate: f64, tolerance_percent: f64) -> Result<Self> {
        ensure!(
            min_rate.is_finite() && min_rate >= 0.0,
            "Invalid minimum penalty rate"
        );
        ensure!(
            max_rate.is_finite() && max_rate >= min_rate,
            "Maximum penalty rate must be finite and at least the minimum rate"
        );
        ensure!(
            tolerance_percent.is_finite() && (0.0..=100.0).contains(&tolerance_percent),
            "Invalid penalty tolerance percentage"
        );
        Ok(Self {
            min_rate,
            max_rate,
            tolerance: (tolerance_percent * NODE_FLOAT_SCALING_FACTOR).round() as u128,
        })
    }

    /// Prices a facility's full slot deviation in contract units, before trade allocation.
    pub(super) fn cost(self, traded: i128, measured: i128) -> Result<u64> {
        const FULL: u128 = 100 * NODE_FLOAT_SCALING_FACTOR as u128;
        let baseline = traded.unsigned_abs();
        let deviation = measured.abs_diff(traded);
        let (actual, allowed) = deviation
            .checked_mul(FULL)
            .zip(baseline.checked_mul(self.tolerance))
            .context("Energy exceeds the range of the tolerance comparison")?;
        if actual <= allowed {
            return Ok(0);
        }
        // Includes a nonzero reading against zero net traded energy.
        let rate = if deviation >= baseline {
            self.max_rate
        } else {
            // Strictly between the tolerance boundary and 100% deviation.
            let fraction =
                (actual - allowed) as f64 / (baseline as f64 * (FULL - self.tolerance) as f64);
            self.min_rate + (self.max_rate - self.min_rate) * fraction
        };
        let cost = (deviation as f64 * rate).round();
        ensure!(
            cost.is_finite() && cost < u64::MAX as f64,
            "Penalty exceeds on-chain range"
        );
        Ok(cost as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_the_full_deviation_at_a_bounded_relative_rate() {
        let policy = PenaltyPolicy::new(0.10, 0.20, 5.0).unwrap();
        let units = NODE_FLOAT_SCALING_FACTOR as i128;
        for (net, measured, expected) in [
            (100, 105, 0),
            (100, 110, 10_526),
            (100, 80, 23_158),
            (50, 60, 11_579),
            (100, 0, 200_000),
            (100, -20, 240_000),
            (0, 0, 0),
            (0, 1, 2_000),
        ] {
            for sign in [1, -1] {
                assert_eq!(
                    policy
                        .cost(sign * net * units, sign * measured * units)
                        .unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn full_tolerance_is_inclusive_and_cost_overflow_is_rejected() {
        let policy = PenaltyPolicy::new(0.10, 0.20, 100.0).unwrap();
        assert_eq!(policy.cost(10_000, 0).unwrap(), 0);
        assert_eq!(policy.cost(10_000, -1).unwrap(), 2_000);
        assert!(PenaltyPolicy::new(0.10, f64::MAX, 0.0)
            .unwrap()
            .cost(10_000, 0)
            .is_err());
    }
}
