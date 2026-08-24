use gsy_execution_engine::timeslot_scheduler::TimeslotScheduler;
use primitives::constants::GLOBAL_CONSTANTS;

fn current_target_timeslot() -> u64 {
    TimeslotScheduler::new(0).calculate_timeslot()
}

fn previous_target_timeslot() -> u64 {
    current_target_timeslot() - GLOBAL_CONSTANTS.time_slot_sec
}

#[test]
fn uses_current_target_without_a_rollover() {
    let current_timeslot = current_target_timeslot();
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(current_timeslot, 2);

    assert_eq!(scheduler.calculate_timeslot(), current_timeslot);
}

#[test]
fn retries_outgoing_timeslot_after_rollover() {
    let previous_timeslot = previous_target_timeslot();
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 2);

    assert_eq!(scheduler.calculate_timeslot(), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 0);
    assert_eq!(scheduler.calculate_timeslot(), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 0);
    assert!(scheduler.calculate_timeslot() > previous_timeslot);
}

#[test]
fn releases_outgoing_timeslot_after_processing_penalties() {
    let previous_timeslot = previous_target_timeslot();
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 2);

    assert_eq!(scheduler.calculate_timeslot(), previous_timeslot);
    scheduler.record_cycle(previous_timeslot, 2);
    assert!(scheduler.calculate_timeslot() > previous_timeslot);
}

#[test]
fn can_disable_rollover_retries() {
    let previous_timeslot = previous_target_timeslot();
    let mut scheduler = TimeslotScheduler::with_initial_timeslot(previous_timeslot, 0);

    assert!(scheduler.calculate_timeslot() > previous_timeslot);
}
