use chrono::{DateTime, Duration, Utc};
use gsy_execution_engine::timeslot_scheduler::TimeslotScheduler;
use primitives::constants::GLOBAL_CONSTANTS;
use std::process::Command;

fn initial_timeslot() -> u64 {
    (1_800_000_000 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

fn time_for_target(timeslot: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(timeslot as i64, 0).unwrap()
        + Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min)
}

#[test]
fn uses_only_current_target_when_retention_is_disabled() {
    let current_timeslot = initial_timeslot();
    let mut scheduler = TimeslotScheduler::new(0);

    for _ in 0..4 {
        assert_eq!(
            scheduler.calculate_timeslot_at(time_for_target(current_timeslot)),
            current_timeslot
        );
    }
}

#[test]
fn target_advances_at_the_exact_slot_boundary_with_the_configured_offset() {
    let initial_timeslot = initial_timeslot();
    let next_timeslot = initial_timeslot + GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(next_timeslot);
    let mut scheduler = TimeslotScheduler::new(0);

    assert_eq!(
        scheduler.calculate_timeslot_at(rollover - Duration::seconds(1)),
        initial_timeslot
    );
    assert_eq!(scheduler.calculate_timeslot_at(rollover), next_timeslot);
}

#[test]
fn alternates_current_and_retained_slots_beyond_two_retries() {
    let current_timeslot = initial_timeslot();
    let previous_timeslot = current_timeslot - GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(current_timeslot);
    let mut scheduler = TimeslotScheduler::new(GLOBAL_CONSTANTS.time_slot_sec);

    for elapsed in 0..8 {
        let now = rollover + Duration::seconds(elapsed);
        assert_eq!(scheduler.calculate_timeslot_at(now), current_timeslot);
        assert_eq!(scheduler.calculate_timeslot_at(now), previous_timeslot);
    }
}

#[test]
fn expires_at_the_deadline_not_after_a_number_of_polls() {
    let current_timeslot = initial_timeslot();
    let previous_timeslot = current_timeslot - GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(current_timeslot);
    let grace_seconds = GLOBAL_CONSTANTS.time_slot_sec / 2;
    let deadline = rollover + Duration::seconds(grace_seconds as i64);
    let mut scheduler = TimeslotScheduler::new(grace_seconds);

    assert_eq!(scheduler.calculate_timeslot_at(rollover), current_timeslot);
    assert_eq!(
        scheduler.calculate_timeslot_at(deadline - Duration::seconds(1)),
        previous_timeslot
    );
    for _ in 0..4 {
        assert_eq!(scheduler.calculate_timeslot_at(deadline), current_timeslot);
    }
}

#[test]
fn restart_recovers_eligible_slots_without_extending_their_deadline() {
    let current_timeslot = initial_timeslot();
    let previous_timeslot = current_timeslot - GLOBAL_CONSTANTS.time_slot_sec;
    let grace_seconds = GLOBAL_CONSTANTS.time_slot_sec / 2;
    let deadline = time_for_target(current_timeslot) + Duration::seconds(grace_seconds as i64);
    let mut scheduler = TimeslotScheduler::new(grace_seconds);

    assert_eq!(
        scheduler.calculate_timeslot_at(deadline - Duration::seconds(1)),
        current_timeslot
    );
    assert_eq!(
        scheduler.calculate_timeslot_at(deadline - Duration::seconds(1)),
        previous_timeslot
    );
    let mut restarted = TimeslotScheduler::new(grace_seconds);
    assert_eq!(restarted.calculate_timeslot_at(deadline), current_timeslot);
    assert_eq!(restarted.calculate_timeslot_at(deadline), current_timeslot);
}

#[test]
fn rotates_multiple_retained_slots_between_current_slot_cycles() {
    let current = initial_timeslot();
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let now = time_for_target(current);
    let mut scheduler = TimeslotScheduler::new(3 * slot);

    for expected in [
        current,
        current - 3 * slot,
        current,
        current - 2 * slot,
        current,
        current - slot,
        current,
        current - 3 * slot,
    ] {
        assert_eq!(scheduler.calculate_timeslot_at(now), expected);
    }
}

#[test]
fn rollover_prioritizes_the_new_target_without_forgetting_older_slots() {
    let initial = initial_timeslot();
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let next = initial + slot;
    let mut scheduler = TimeslotScheduler::new(2 * slot);

    // The next turn would have been retained work, but a new target takes priority.
    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(initial)),
        initial
    );
    for expected in [next, initial - slot, next, initial, next, initial - slot] {
        assert_eq!(
            scheduler.calculate_timeslot_at(time_for_target(next)),
            expected
        );
    }
}

#[test]
fn missed_rollovers_reconstruct_only_the_still_eligible_window() {
    let initial = initial_timeslot();
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let current = initial + 5 * slot;
    let mut scheduler = TimeslotScheduler::new(2 * slot);

    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(initial)),
        initial
    );
    for expected in [current, current - 2 * slot, current, current - slot] {
        assert_eq!(
            scheduler.calculate_timeslot_at(time_for_target(current)),
            expected
        );
    }
}

#[test]
fn expired_retained_cursor_is_skipped_even_between_slot_boundaries() {
    let current = initial_timeslot();
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let grace_seconds = slot + slot / 2;
    let now = time_for_target(current);
    let mut scheduler = TimeslotScheduler::new(grace_seconds);

    for expected in [current, current - 2 * slot, current, current - slot] {
        assert_eq!(scheduler.calculate_timeslot_at(now), expected);
    }
    let deadline = now + Duration::seconds((slot / 2) as i64);
    for expected in [current, current - slot, current, current - slot] {
        assert_eq!(scheduler.calculate_timeslot_at(deadline), expected);
    }
}

#[test]
fn backward_clock_change_never_selects_a_future_slot() {
    let current = initial_timeslot();
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let mut scheduler = TimeslotScheduler::new(slot);

    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current)),
        current
    );
    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current)),
        current - slot
    );
    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current - slot)),
        current - slot
    );
    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current - slot)),
        current - 2 * slot
    );
}

#[test]
fn grace_window_does_not_underflow_near_the_epoch() {
    let mut scheduler = TimeslotScheduler::new(u64::MAX);

    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(0) - Duration::seconds(1)),
        0
    );
    assert_eq!(scheduler.calculate_timeslot_at(time_for_target(0)), 0);
}

#[test]
fn invalid_grace_configuration_fails_before_starting_execution_cycles() {
    for value in ["", "invalid", "-1", "1.5", "18446744073709551616"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gsy-execution-engine"))
            .arg("web3")
            .env("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS", value)
            .output()
            .expect("execution engine should start");
        let messages = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.status.success(), "{messages}");
        assert!(
            messages
                .contains("EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS must be a non-negative integer"),
            "{messages}"
        );
        assert!(
            !messages.contains("Execution cycle for timeslot"),
            "{messages}"
        );
    }
}

#[test]
fn expired_slot_is_the_latest_one_past_its_grace_period_and_already_delivered() {
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let offset_seconds = GLOBAL_CONSTANTS.execution_engine_offset_min * 60;
    for grace in [0, slot / 2, slot, 3 * slot] {
        let scheduler = TimeslotScheduler::new(grace);
        for elapsed in [0, 1, slot / 2, slot - 1, slot, 2 * slot + 1] {
            let now = time_for_target(initial_timeslot()) + Duration::seconds(elapsed as i64);
            let now_seconds = now.timestamp() as u64;
            let target = (now.timestamp() - offset_seconds) as u64;
            let oldest_eligible = ((target - grace) / slot) * slot;
            let is_expired = |candidate: u64| {
                // No longer selected for retries, and delivered at least one grace period ago.
                candidate < oldest_eligible && candidate + slot + grace <= now_seconds
            };

            let expired = scheduler.expired_timeslot_at(now).unwrap();
            assert_eq!(expired % slot, 0);
            assert!(is_expired(expired), "grace={grace}, elapsed={elapsed}");
            assert!(
                !is_expired(expired + slot),
                "grace={grace}, elapsed={elapsed}"
            );
        }
    }
}

#[test]
fn expired_slot_is_never_selected_for_a_regular_cycle() {
    let slot = GLOBAL_CONSTANTS.time_slot_sec;
    let mut scheduler = TimeslotScheduler::new(2 * slot);
    let now = time_for_target(initial_timeslot()) + Duration::seconds(1);
    let expired = scheduler.expired_timeslot_at(now).unwrap();
    for _ in 0..8 {
        assert!(scheduler.calculate_timeslot_at(now) > expired);
    }
}

#[test]
fn no_slot_has_expired_near_the_epoch() {
    let scheduler = TimeslotScheduler::new(GLOBAL_CONSTANTS.time_slot_sec);
    assert_eq!(
        scheduler.expired_timeslot_at(DateTime::from_timestamp(0, 0).unwrap()),
        None
    );
}
