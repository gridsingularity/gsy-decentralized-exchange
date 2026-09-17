use chrono::{Duration, Utc};
use primitives::constants::GLOBAL_CONSTANTS;
use tracing::info;

pub const DEFAULT_ROLLOVER_RETRY_LIMIT: u32 = 2;

#[derive(Debug)]
struct PendingTimeslot {
    timeslot: u64,
    retries_remaining: u32,
}

#[derive(Debug)]
pub struct TimeslotScheduler {
    latest_target_timeslot: u64,
    pending_timeslot: Option<PendingTimeslot>,
    rollover_retry_limit: u32,
}

impl TimeslotScheduler {
    pub fn new(rollover_retry_limit: u32) -> Self {
        Self::with_initial_timeslot(generate_target_timeslot(), rollover_retry_limit)
    }

    pub fn with_initial_timeslot(initial_timeslot: u64, rollover_retry_limit: u32) -> Self {
        Self {
            latest_target_timeslot: initial_timeslot,
            pending_timeslot: None,
            rollover_retry_limit,
        }
    }

    pub fn calculate_timeslot(&mut self) -> u64 {
        let current_target_timeslot = generate_target_timeslot();

        if current_target_timeslot != self.latest_target_timeslot {
            if self.rollover_retry_limit > 0 {
                info!(
                    "Target timeslot advanced from {} to {}; retaining {} for up to {} retries",
                    self.latest_target_timeslot,
                    current_target_timeslot,
                    self.latest_target_timeslot,
                    self.rollover_retry_limit
                );
                self.pending_timeslot = Some(PendingTimeslot {
                    timeslot: self.latest_target_timeslot,
                    retries_remaining: self.rollover_retry_limit,
                });
            }
            self.latest_target_timeslot = current_target_timeslot;
        }

        self.pending_timeslot
            .as_ref()
            .map(|pending| pending.timeslot)
            .unwrap_or(current_target_timeslot)
    }

    pub fn record_cycle(&mut self, timeslot: u64, processed_penalties: usize) {
        let Some(pending) = self.pending_timeslot.as_mut() else {
            return;
        };
        if pending.timeslot != timeslot {
            return;
        }

        if processed_penalties > 0 {
            info!(
                "Finished retained timeslot {} after processing {} penalties",
                timeslot, processed_penalties
            );
            self.pending_timeslot = None;
        } else if pending.retries_remaining <= 1 {
            info!(
                "Finished retained timeslot {} after exhausting rollover retries",
                timeslot
            );
            self.pending_timeslot = None;
        } else {
            pending.retries_remaining -= 1;
            info!(
                "Retained timeslot {} has {} retries remaining",
                timeslot, pending.retries_remaining
            );
        }
    }
}

fn generate_target_timeslot() -> u64 {
    let now = Utc::now();
    let previous = now - Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min);

    (previous.timestamp() as u64 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}
