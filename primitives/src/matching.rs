use crate::MatchingAlgorithm;
use std::env;
use std::ops::RangeInclusive;

const DEFAULT_PAY_AS_BID_BLOCK_INTERVAL: u64 = 4;
const DEFAULT_PAY_AS_CLEAR_BLOCK_INTERVAL: u64 = 64;

/// Overrides the block interval of `pay_as_bid` markets.
pub const PAY_AS_BID_BLOCK_INTERVAL_ENV: &str = "MATCHING_ENGINE_PAY_AS_BID_BLOCK_INTERVAL";
/// Overrides the block interval of `pay_as_clear` markets.
pub const PAY_AS_CLEAR_BLOCK_INTERVAL_ENV: &str = "MATCHING_ENGINE_PAY_AS_CLEAR_BLOCK_INTERVAL";

/// Number of blocks between two matching cycles of markets cleared with
/// `algorithm`. The override variable must hold a positive integer; any other
/// value falls back to the default.
pub fn matching_block_interval(algorithm: MatchingAlgorithm) -> u64 {
    let (variable, default) = match algorithm {
        MatchingAlgorithm::PayAsClear => (
            PAY_AS_CLEAR_BLOCK_INTERVAL_ENV,
            DEFAULT_PAY_AS_CLEAR_BLOCK_INTERVAL,
        ),
        // `amm` is never matched, so it shares the pay-as-bid cadence.
        MatchingAlgorithm::PayAsBid | MatchingAlgorithm::AMM => (
            PAY_AS_BID_BLOCK_INTERVAL_ENV,
            DEFAULT_PAY_AS_BID_BLOCK_INTERVAL,
        ),
    };
    env::var(variable)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// Whether markets cleared with `algorithm` are matched after `new_blocks`
/// were mined: one of the blocks is a multiple of the algorithm's interval.
/// Taking all blocks since the last check means no boundary is missed when
/// several blocks are mined between two checks.
pub fn is_matching_due(algorithm: MatchingAlgorithm, new_blocks: &RangeInclusive<u64>) -> bool {
    let interval = matching_block_interval(algorithm);
    new_blocks.end() / interval > new_blocks.start().saturating_sub(1) / interval
}
