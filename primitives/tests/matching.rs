use primitives::matching::ClearingSchedule;

// A quarter-hour boundary.
const OPENING: u64 = 1_700_000_100;
const MINUTE: u64 = 60;

fn schedule(clearings_per_market: u64) -> ClearingSchedule {
    ClearingSchedule {
        clearings_per_market,
        final_clearing_lead_seconds: 20,
    }
}

#[test]
fn a_single_clearing_takes_place_the_lead_time_before_the_close() {
    let closing = OPENING + 120 * MINUTE;

    assert_eq!(
        schedule(1).clearing_times(OPENING, closing),
        vec![closing - 20]
    );
    assert_eq!(
        ClearingSchedule::default().clearing_times(OPENING, closing),
        vec![closing - 20]
    );
}

#[test]
fn clearings_are_spread_evenly_and_the_last_one_keeps_the_lead_time() {
    let closing = OPENING + 15 * MINUTE;

    assert_eq!(
        schedule(3).clearing_times(OPENING, closing),
        vec![
            OPENING + 5 * MINUTE,
            OPENING + 10 * MINUTE,
            OPENING + 15 * MINUTE - 20
        ]
    );
}

#[test]
fn the_window_of_the_market_sets_the_spacing() {
    let closing = OPENING + 120 * MINUTE;

    assert_eq!(
        schedule(3).clearing_times(OPENING, closing),
        vec![
            OPENING + 40 * MINUTE,
            OPENING + 80 * MINUTE,
            OPENING + 120 * MINUTE - 20
        ]
    );
}

#[test]
fn no_clearing_takes_place_inside_the_lead_time() {
    // 60 clearings in 15 minutes are 15 seconds apart, so the last two fall
    // inside the 20-second lead time and collapse into one.
    let closing = OPENING + 15 * MINUTE;
    let times = schedule(60).clearing_times(OPENING, closing);

    assert_eq!(times.len(), 59);
    assert_eq!(times.first(), Some(&(OPENING + 15)));
    assert_eq!(times.last(), Some(&(closing - 20)));
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn a_market_shorter_than_the_lead_time_is_not_cleared() {
    assert!(schedule(1).clearing_times(OPENING, OPENING + 20).is_empty());
    assert!(schedule(1).clearing_times(OPENING, OPENING).is_empty());
    assert!(schedule(1).clearing_times(OPENING, OPENING - 1).is_empty());
    assert_eq!(
        schedule(3).clearing_times(OPENING, OPENING + 21),
        vec![OPENING + 1]
    );
}

#[test]
fn a_clearing_is_due_once_in_the_window_that_contains_its_time() {
    let closing = OPENING + 15 * MINUTE;
    let schedule = schedule(3);
    let first = OPENING + 5 * MINUTE;

    assert!(!schedule.is_clearing_due(OPENING, closing, OPENING, first - 1));
    assert!(schedule.is_clearing_due(OPENING, closing, first - 1, first));
    assert!(!schedule.is_clearing_due(OPENING, closing, first, first + 1));
    assert!(schedule.is_clearing_due(OPENING, closing, closing - 21, closing - 20));
    assert!(!schedule.is_clearing_due(OPENING, closing, closing - 20, closing + 100));
}
