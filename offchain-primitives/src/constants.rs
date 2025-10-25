
pub struct Constants;

impl Constants {
    pub const TIME_SLOT_SEC: u64 = 900;
    pub const EXECUTION_ENGINE_OFFSET_MIN: i64 = -120;
    pub const SPOT_MARKET_OPEN_OFFSET_MIN: i64 = -120;
    pub const SPOT_MARKET_CLOSE_OFFSET_MIN: i64 = -60;
    pub const FLEX_MARKET_OPEN_OFFSET_MIN: i64 = -15;
    pub const FLEX_MARKET_CLOSE_OFFSET_MIN: i64 = 0;
    pub const SETTLEMENT_MARKET_OPEN_OFFSET_MIN: i64 = 30;
    pub const SETTLEMENT_MARKET_CLOSE_OFFSET_MIN: i64 = 60;
}
