use crate::utils::read_env_or;
use once_cell::sync::Lazy;

pub struct Constants {
    pub TIME_SLOT_SEC: u64,
    pub EXECUTION_ENGINE_OFFSET_MIN: i64,
    pub SPOT_MARKET_OPEN_OFFSET_MIN: i64,
    pub SPOT_MARKET_CLOSE_OFFSET_MIN: i64,
    pub FLEX_MARKET_OPEN_OFFSET_MIN: i64,
    pub FLEX_MARKET_CLOSE_OFFSET_MIN: i64,
    pub SETTLEMENT_MARKET_OPEN_OFFSET_MIN: i64,
    pub SETTLEMENT_MARKET_CLOSE_OFFSET_MIN: i64,
}

impl Constants {
    fn new() -> Self {
        Self {
            TIME_SLOT_SEC: read_env_or("TIME_SLOT_SEC", 900),
            EXECUTION_ENGINE_OFFSET_MIN: read_env_or("EXECUTION_ENGINE_OFFSET_MIN", -120),
            SPOT_MARKET_OPEN_OFFSET_MIN: read_env_or("SPOT_MARKET_OPEN_OFFSET_MIN", -180),
            SPOT_MARKET_CLOSE_OFFSET_MIN: read_env_or("SPOT_MARKET_CLOSE_OFFSET_MIN", -60),
            FLEX_MARKET_OPEN_OFFSET_MIN: read_env_or("FLEX_MARKET_OPEN_OFFSET_MIN", -15),
            FLEX_MARKET_CLOSE_OFFSET_MIN: read_env_or("FLEX_MARKET_CLOSE_OFFSET_MIN", 0),
            SETTLEMENT_MARKET_OPEN_OFFSET_MIN: read_env_or("SETTLEMENT_MARKET_OPEN_OFFSET_MIN", -60),
            SETTLEMENT_MARKET_CLOSE_OFFSET_MIN: read_env_or("SETTLEMENT_MARKET_CLOSE_OFFSET_MIN", -30),
        }
    }
}

pub static GlobalConstants: Lazy<Constants> = Lazy::new(Constants::new);
