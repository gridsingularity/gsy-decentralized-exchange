use gsy_community_client::time_utils::start_of_previous_day;

const SECS_PER_DAY: u64 = 86_400;

#[test]
fn start_of_previous_day_floors_and_subtracts_one_day() {
    // 2024-06-15T12:34:56Z
    let now = 1_718_454_896u64;
    // 2024-06-14T00:00:00Z
    let expected = 1_718_323_200u64;
    assert_eq!(start_of_previous_day(now), expected);
}

#[test]
fn start_of_previous_day_at_exact_midnight() {
    // 2024-06-15T00:00:00Z
    let midnight = 1_718_409_600u64;
    assert_eq!(start_of_previous_day(midnight), midnight - SECS_PER_DAY);
}

#[test]
fn start_of_previous_day_one_second_before_midnight() {
    let one_second_before_midnight = 1_718_409_600u64 - 1;
    let expected = ((one_second_before_midnight / SECS_PER_DAY) * SECS_PER_DAY) - SECS_PER_DAY;
    assert_eq!(start_of_previous_day(one_second_before_midnight), expected);
}

#[test]
fn start_of_previous_day_is_idempotent_across_the_day() {
    let start_of_day = 1_718_409_600u64;
    for offset in [0u64, 1, 3_600, SECS_PER_DAY - 1] {
        assert_eq!(
            start_of_previous_day(start_of_day + offset),
            start_of_day - SECS_PER_DAY
        );
    }
}
