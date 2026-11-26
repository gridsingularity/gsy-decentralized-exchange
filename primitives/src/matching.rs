//! When the matching engine runs.
//!
//! The matching engine clears each market a fixed number of times, spread
//! evenly over the time the market is open, whatever the market's matching
//! algorithm. The last clearing takes place a lead time before the closing
//! time, so that its matches are recorded while the market still accepts
//! them.

use std::env;
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CLEARINGS_PER_MARKET: &str = "CLEARINGS_PER_MARKET";
pub const FINAL_CLEARING_LEAD_SECONDS: &str = "FINAL_CLEARING_LEAD_SECONDS";

const DEFAULT_CLEARINGS_PER_MARKET: u64 = 1;
const DEFAULT_FINAL_CLEARING_LEAD_SECONDS: u64 = 20;

/// When the markets are cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClearingSchedule {
    /// How many times each market is cleared while it is open.
    pub clearings_per_market: u64,
    /// How long before the closing time the last clearing takes place.
    pub final_clearing_lead_seconds: u64,
}

impl Default for ClearingSchedule {
    fn default() -> Self {
        Self {
            clearings_per_market: DEFAULT_CLEARINGS_PER_MARKET,
            final_clearing_lead_seconds: DEFAULT_FINAL_CLEARING_LEAD_SECONDS,
        }
    }
}

impl ClearingSchedule {
    /// Reads the schedule from the environment. An unset or empty variable
    /// takes its default.
    pub fn from_env() -> Result<Self, String> {
        let clearings_per_market =
            read_env::<u64>(CLEARINGS_PER_MARKET)?.unwrap_or(DEFAULT_CLEARINGS_PER_MARKET);
        if clearings_per_market == 0 {
            return Err(format!(
                "Invalid {}: must be greater than 0",
                CLEARINGS_PER_MARKET
            ));
        }
        let final_clearing_lead_seconds = read_env::<u64>(FINAL_CLEARING_LEAD_SECONDS)?
            .unwrap_or(DEFAULT_FINAL_CLEARING_LEAD_SECONDS);

        Ok(Self {
            clearings_per_market,
            final_clearing_lead_seconds,
        })
    }

    /// The times at which a market open from `opening_time` to `closing_time`
    /// is cleared, in ascending order.
    ///
    /// The open window is divided into `clearings_per_market` equal parts and
    /// the market is cleared at the end of each part. No clearing takes place
    /// later than the lead time before the closing time: the last clearing is
    /// moved there, and so are earlier ones that would fall behind it. The
    /// market is then cleared fewer times. A market that is open for no longer
    /// than the lead time is not cleared.
    pub fn clearing_times(&self, opening_time: u64, closing_time: u64) -> Vec<u64> {
        let latest = closing_time.saturating_sub(self.final_clearing_lead_seconds);
        if closing_time <= opening_time || latest <= opening_time {
            return Vec::new();
        }

        let duration = (closing_time - opening_time) as u128;
        let parts = self.clearings_per_market as u128;
        let mut times: Vec<u64> = (1..=parts)
            .map(|part| opening_time + (duration * part / parts) as u64)
            .map(|time| time.min(latest))
            .filter(|time| *time > opening_time)
            .collect();
        times.dedup();
        times
    }

    /// Whether a market has a clearing time in `(after, until]`.
    pub fn is_clearing_due(
        &self,
        opening_time: u64,
        closing_time: u64,
        after: u64,
        until: u64,
    ) -> bool {
        self.clearing_times(opening_time, closing_time)
            .into_iter()
            .any(|time| after < time && time <= until)
    }
}

pub fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock is before the Unix epoch")
        .as_secs()
}

fn read_env<T>(name: &str) -> Result<Option<T>, String>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => value
            .trim()
            .parse::<T>()
            .map(Some)
            .map_err(|error| format!("Invalid {}: {}", name, error)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(format!("Invalid {}: {}", name, error)),
    }
}
