use gsy_offchain_primitives::constants::GlobalConstants;
use std::time::{SystemTime, UNIX_EPOCH};

pub const TIMESLOT_MINUTES: u16 = 15;

pub fn get_current_timestamp_in_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn get_last_and_next_timeslot() -> (u64, u64) {
    const TIMESLOT_SECS: u64 = (TIMESLOT_MINUTES * 60) as u64;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs_since_last_timeslot = now % ((TIMESLOT_MINUTES * 60) as u64);
    let last_quarter = now - secs_since_last_timeslot;
    let next_quarter = last_quarter + TIMESLOT_SECS;
    (last_quarter, next_quarter)
}

/// Return every delivery timeslot whose spot market is currently open for order submission
/// at now, using the same open/close offsets the market orchestrator uses.
pub fn open_spot_market_timeslots(now: u64) -> Vec<u64> {
    let slot = GlobalConstants.TIME_SLOT_SEC;
    let look_ahead = GlobalConstants.SPOT_MARKET_OPEN_OFFSET_MIN.unsigned_abs() * 60;
    let mut timeslots = Vec::new();
    let mut timeslot = (now / slot) * slot;
    while timeslot <= now + look_ahead {
        let (open_time, close_time) = GlobalConstants.spot_market_window(timeslot);
        if now >= open_time && now < close_time {
            timeslots.push(timeslot);
        }
        timeslot += slot;
    }
    timeslots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_spot_market_timeslots_lie_within_their_window() {
        let now = 1_700_000_000u64;
        let slots = open_spot_market_timeslots(now);
        assert!(!slots.is_empty(), "expected several markets open in parallel");
        let mut previous: Option<u64> = None;
        for timeslot in &slots {
            let (open_time, close_time) = GlobalConstants.spot_market_window(*timeslot);
            assert!(
                now >= open_time && now < close_time,
                "timeslot {} is not open at {}",
                timeslot,
                now
            );
            assert_eq!(timeslot % GlobalConstants.TIME_SLOT_SEC, 0);
            if let Some(previous) = previous {
                assert_eq!(
                    *timeslot,
                    previous + GlobalConstants.TIME_SLOT_SEC,
                    "open timeslots must be contiguous"
                );
            }
            previous = Some(*timeslot);
        }
    }
}
