use chrono::{DateTime, Duration, Utc};
use gsy_execution_engine::timeslot_scheduler::TimeslotScheduler;
use primitives::constants::GLOBAL_CONSTANTS;

fn initial_timeslot() -> u64 {
    (1_800_000_000 / GLOBAL_CONSTANTS.time_slot_sec) * GLOBAL_CONSTANTS.time_slot_sec
}

fn time_for_target(timeslot: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(timeslot as i64, 0).unwrap()
        + Duration::minutes(GLOBAL_CONSTANTS.execution_engine_offset_min)
}

#[test]
fn uses_current_target_without_a_rollover() {
    let current_timeslot = initial_timeslot();
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(current_timeslot, 2);

    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current_timeslot)),
        current_timeslot
    );
}

#[test]
fn target_advances_at_the_exact_slot_boundary_with_the_configured_offset() {
    let initial_timeslot = initial_timeslot();
    let next_timeslot = initial_timeslot + GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(next_timeslot);
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(initial_timeslot, 0);

    assert_eq!(
        scheduler.calculate_timeslot_at(rollover - Duration::seconds(1)),
        initial_timeslot
    );
    assert_eq!(scheduler.calculate_timeslot_at(rollover), next_timeslot);
}

#[test]
fn retries_outgoing_timeslot_after_rollover() {
    let previous_timeslot = initial_timeslot();
    let current_timeslot = previous_timeslot + GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(current_timeslot);
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 2);

    assert_eq!(scheduler.calculate_timeslot_at(rollover), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 0);
    assert_eq!(scheduler.calculate_timeslot_at(rollover), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 0);
    assert_eq!(scheduler.calculate_timeslot_at(rollover), current_timeslot);
}

#[test]
fn releases_outgoing_timeslot_after_processing_penalties() {
    let previous_timeslot = initial_timeslot();
    let current_timeslot = previous_timeslot + GLOBAL_CONSTANTS.time_slot_sec;
    let rollover = time_for_target(current_timeslot);
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 2);

    assert_eq!(scheduler.calculate_timeslot_at(rollover), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 2);
    assert_eq!(scheduler.calculate_timeslot_at(rollover), current_timeslot);
}

#[test]
fn can_disable_rollover_retries() {
    let previous_timeslot = initial_timeslot();
    let current_timeslot = previous_timeslot + GLOBAL_CONSTANTS.time_slot_sec;
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 0);

    assert_eq!(
        scheduler.calculate_timeslot_at(time_for_target(current_timeslot)),
        current_timeslot
    );
}
