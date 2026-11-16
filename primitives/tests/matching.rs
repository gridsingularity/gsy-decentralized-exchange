use primitives::matching::{
    is_matching_due, matching_block_interval, PAY_AS_BID_BLOCK_INTERVAL_ENV,
    PAY_AS_CLEAR_BLOCK_INTERVAL_ENV,
};
use primitives::MatchingAlgorithm;
use std::env;
use std::sync::{Mutex, PoisonError};

/// The tests change process-wide environment variables, so they run one at a
/// time.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const REMOVED_GLOBAL_OVERRIDE_ENV: &str = "MATCHING_ENGINE_BLOCK_INTERVAL";

/// Runs `check` with exactly `variables` of the block interval variables set.
fn with_variables(variables: &[(&str, &str)], check: impl FnOnce()) {
    let _guard = ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let all = [
        PAY_AS_BID_BLOCK_INTERVAL_ENV,
        PAY_AS_CLEAR_BLOCK_INTERVAL_ENV,
        REMOVED_GLOBAL_OVERRIDE_ENV,
    ];
    for variable in all {
        env::remove_var(variable);
    }
    for (variable, value) in variables {
        env::set_var(variable, value);
    }
    check();
    for variable in all {
        env::remove_var(variable);
    }
}

#[test]
fn defaults_depend_on_the_algorithm() {
    with_variables(&[], || {
        assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsBid), 4);
        assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsClear), 64);
        assert_eq!(matching_block_interval(MatchingAlgorithm::AMM), 4);
    });
}

#[test]
fn each_algorithm_reads_only_its_own_override() {
    with_variables(
        &[
            (PAY_AS_BID_BLOCK_INTERVAL_ENV, "2"),
            (PAY_AS_CLEAR_BLOCK_INTERVAL_ENV, "16"),
        ],
        || {
            assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsBid), 2);
            assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsClear), 16);
        },
    );
    with_variables(&[(PAY_AS_BID_BLOCK_INTERVAL_ENV, "2")], || {
        assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsClear), 64);
    });
}

#[test]
fn invalid_overrides_fall_back_to_the_default() {
    for value in ["", "0", "-4", "abc"] {
        with_variables(&[(PAY_AS_CLEAR_BLOCK_INTERVAL_ENV, value)], || {
            assert_eq!(
                matching_block_interval(MatchingAlgorithm::PayAsClear),
                64,
                "{value:?}"
            );
        });
    }
}

#[test]
fn a_boundary_is_due_once_also_across_skipped_blocks() {
    with_variables(&[], || {
        assert!(is_matching_due(MatchingAlgorithm::PayAsBid, &(4..=4)));
        assert!(!is_matching_due(MatchingAlgorithm::PayAsBid, &(5..=7)));
        // Block 64 was not polled, but lies between the two checks.
        assert!(is_matching_due(MatchingAlgorithm::PayAsClear, &(61..=66)));
        assert!(!is_matching_due(MatchingAlgorithm::PayAsClear, &(65..=127)));
        assert!(is_matching_due(MatchingAlgorithm::PayAsClear, &(65..=128)));
    });
}

#[test]
fn the_removed_global_override_is_ignored() {
    with_variables(&[(REMOVED_GLOBAL_OVERRIDE_ENV, "8")], || {
        assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsBid), 4);
        assert_eq!(matching_block_interval(MatchingAlgorithm::PayAsClear), 64);
    });
}
