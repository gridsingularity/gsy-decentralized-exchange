use primitives::matching::{ClearingSchedule, CLEARINGS_PER_MARKET, FINAL_CLEARING_LEAD_SECONDS};
use std::env;
use std::sync::Mutex;

// The environment is shared by all tests of this binary.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const OLD_CLEARINGS_PER_MARKET: &str = "PAY_AS_CLEAR_CLEARINGS_PER_MARKET";
const OLD_FINAL_CLEARING_LEAD_SECONDS: &str = "PAY_AS_CLEAR_FINAL_CLEARING_LEAD_SECONDS";

/// Reads the schedule with only `variables` set.
fn schedule_from_env(variables: &[(&str, &str)]) -> Result<ClearingSchedule, String> {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let names = [
        CLEARINGS_PER_MARKET,
        FINAL_CLEARING_LEAD_SECONDS,
        OLD_CLEARINGS_PER_MARKET,
        OLD_FINAL_CLEARING_LEAD_SECONDS,
    ];
    for name in names {
        env::remove_var(name);
    }
    for (name, value) in variables {
        env::set_var(name, value);
    }
    let schedule = ClearingSchedule::from_env();
    for name in names {
        env::remove_var(name);
    }
    schedule
}

#[test]
fn the_schedule_defaults_to_one_clearing_20_seconds_before_the_close() {
    for variables in [
        vec![],
        vec![
            (CLEARINGS_PER_MARKET, ""),
            (FINAL_CLEARING_LEAD_SECONDS, " "),
        ],
    ] {
        assert_eq!(
            schedule_from_env(&variables),
            Ok(ClearingSchedule {
                clearings_per_market: 1,
                final_clearing_lead_seconds: 20,
            })
        );
    }
}

#[test]
fn the_schedule_is_read_from_its_variables() {
    assert_eq!(
        schedule_from_env(&[
            (CLEARINGS_PER_MARKET, "3"),
            (FINAL_CLEARING_LEAD_SECONDS, " 30 ")
        ]),
        Ok(ClearingSchedule {
            clearings_per_market: 3,
            final_clearing_lead_seconds: 30,
        })
    );
}

#[test]
fn an_invalid_schedule_names_its_variable() {
    for (name, value) in [
        (CLEARINGS_PER_MARKET, "0"),
        (CLEARINGS_PER_MARKET, "three"),
        (FINAL_CLEARING_LEAD_SECONDS, "-1"),
    ] {
        let error = schedule_from_env(&[(name, value)]).unwrap_err();
        assert!(error.starts_with(&format!("Invalid {name}")), "{error}");
    }
}

#[test]
fn the_former_pay_as_clear_names_are_ignored() {
    assert_eq!(
        schedule_from_env(&[
            (OLD_CLEARINGS_PER_MARKET, "0"),
            (OLD_FINAL_CLEARING_LEAD_SECONDS, "30"),
        ]),
        Ok(ClearingSchedule::default())
    );
}

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
