use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use primitives::constants::GLOBAL_CONSTANTS;
use std::env;
use tracing::{info, warn};

pub const DEFAULT_ROLLOVER_GRACE_SECONDS: u64 = 900;

#[derive(Debug)]
pub struct TimeslotScheduler {
    latest_target_timeslot: Option<u64>,
    next_retained_timeslot: Option<u64>,
    retained_turn: bool,
    rollover_grace_seconds: u64,
}

impl TimeslotScheduler {
    pub fn new(rollover_grace_seconds: u64) -> Self {
        Self {
            latest_target_timeslot: None,
            next_retained_timeslot: None,
            retained_turn: false,
            rollover_grace_seconds,
        }
    }

    pub fn from_env() -> Result<Self> {
        let grace_seconds = match env::var("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS") {
            Ok(value) => value.parse::<u64>().context(
                "EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS must be a non-negative integer",
            )?,
            Err(env::VarError::NotPresent) => DEFAULT_ROLLOVER_GRACE_SECONDS,
            Err(error) => return Err(error.into()),
        };
        if env::var_os("EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT").is_some() {
            warn!(
                "EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT is ignored; use EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS instead"
            );
        }
        info!("Execution rollover grace period: {} seconds", grace_seconds);
        Ok(Self::new(grace_seconds))
    }

    pub fn calculate_timeslot(&mut self) -> u64 {
        self.calculate_timeslot_at(Utc::now())
    }

    /// Select a slot at an explicit time, allowing rollover tests without sleeping.
    pub fn calculate_timeslot_at(&mut self, now: DateTime<Utc>) -> u64 {
        let target_time = now - Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);
        let target_timestamp = target_time.timestamp().max(0) as u64;
        let slot_duration = GLOBAL_CONSTANTS.time_slot_sec;
        let current_target = (target_timestamp / slot_duration) * slot_duration;

        // A slot remains eligible until its rollover + grace. Derive the window
        // from time so restarts and delayed polling cannot extend the deadline.
        let oldest_eligible = (target_timestamp.saturating_sub(self.rollover_grace_seconds)
            / slot_duration)
            * slot_duration;

        if self.latest_target_timeslot != Some(current_target)
            || !self.retained_turn
            || oldest_eligible == current_target
        {
            self.latest_target_timeslot = Some(current_target);
            self.retained_turn = true;
            return current_target;
        }

        let retained = self
            .next_retained_timeslot
            .filter(|slot| *slot >= oldest_eligible && *slot < current_target)
            .unwrap_or(oldest_eligible);
        self.next_retained_timeslot = Some(retained + slot_duration);
        self.retained_turn = false;
        retained
    }
}
