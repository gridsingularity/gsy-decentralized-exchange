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
        const HUNDRED_PERCENT: u128 = 100 * NODE_FLOAT_SCALING_FACTOR as u128;
        let abs_traded_energy = traded.unsigned_abs();
        let deviation = measured.abs_diff(traded);
        let actual_deviation = deviation
            .checked_mul(HUNDRED_PERCENT)
            .context("Deviation exceeds the range of the tolerance comparison")?;
        let allowed_deviation = abs_traded_energy
            .checked_mul(self.tolerance)
            .context("Traded energy exceeds the range of the tolerance comparison")?;
        if actual_deviation <= allowed_deviation {
            return Ok(0);
        }
        // Includes a nonzero reading against zero net traded energy.
        let rate = if deviation >= abs_traded_energy {
            self.max_rate
        } else {
            // Strictly between the tolerance boundary and 100% deviation.
            let fraction = (actual_deviation - allowed_deviation) as f64
                / (abs_traded_energy as f64 * (HUNDRED_PERCENT - self.tolerance) as f64);
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
#[path = "../../tests/primitives/penalty_policy.rs"]
mod tests;
