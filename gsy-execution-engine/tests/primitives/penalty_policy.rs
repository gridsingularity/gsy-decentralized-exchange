use super::*;

#[test]
fn charges_the_full_deviation_at_a_bounded_relative_rate() {
    let policy = PenaltyPolicy::new(0.10, 0.20, 5.0).unwrap();
    let units = NODE_FLOAT_SCALING_FACTOR as i128;
    for (net, measured, expected) in [
        (100, 105, 0),
        (100, 110, 10_526),
        (100, 80, 23_158),
        (50, 60, 11_579),
        (100, 0, 200_000),
        (100, -20, 240_000),
        (0, 0, 0),
        (0, 1, 2_000),
    ] {
        for sign in [1, -1] {
            assert_eq!(
                policy
                    .cost(sign * net * units, sign * measured * units)
                    .unwrap(),
                expected
            );
        }
    }
}

#[test]
fn full_tolerance_is_inclusive_and_cost_overflow_is_rejected() {
    let policy = PenaltyPolicy::new(0.10, 0.20, 100.0).unwrap();
    assert_eq!(policy.cost(10_000, 0).unwrap(), 0);
    assert_eq!(policy.cost(10_000, -1).unwrap(), 2_000);
    assert!(PenaltyPolicy::new(0.10, f64::MAX, 0.0)
        .unwrap()
        .cost(10_000, 0)
        .is_err());
}
