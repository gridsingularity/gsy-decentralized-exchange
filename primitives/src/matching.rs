use crate::MatchingAlgorithm;
use std::env;

const DEFAULT_PAY_AS_BID_BLOCK_INTERVAL: u64 = 4;
const DEFAULT_PAY_AS_CLEAR_BLOCK_INTERVAL: u64 = 64;

pub fn matching_block_interval() -> u64 {
    let matching_algorithm = env::var("MATCHING_ALGORITHM")
        .ok()
        .and_then(|value| value.parse::<MatchingAlgorithm>().ok())
        .unwrap_or_default();

    env::var("MATCHING_ENGINE_BLOCK_INTERVAL")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(match matching_algorithm {
            MatchingAlgorithm::PayAsClear => DEFAULT_PAY_AS_CLEAR_BLOCK_INTERVAL,
            _ => DEFAULT_PAY_AS_BID_BLOCK_INTERVAL,
        })
}
