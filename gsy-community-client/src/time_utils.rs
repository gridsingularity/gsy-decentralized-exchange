use std::time::{SystemTime, UNIX_EPOCH};


pub const TIMESLOT_MINUTES: u16 = 15;

pub fn get_current_timestamp_in_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}


pub fn get_last_and_next_timeslot() -> (u64, u64) {
    const TIMESLOT_SECS: u64 = (TIMESLOT_MINUTES * 60) as u64;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let secs_since_last_timeslot = now % ((TIMESLOT_MINUTES * 60) as u64);
    let last_quarter = now - secs_since_last_timeslot;
    let next_quarter = last_quarter + TIMESLOT_SECS;
    (last_quarter, next_quarter)
}
