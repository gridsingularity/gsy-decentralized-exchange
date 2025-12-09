use crate::utils::read_env_or;
use once_cell::sync::Lazy;

pub struct Constants {
	pub time_slot_sec: u64,
	pub execution_engine_offset_min: i64,
	pub spot_market_open_offset_min: i64,
	pub spot_market_close_offset_min: i64,
	pub flex_market_open_offset_min: i64,
	pub flex_market_close_offset_min: i64,
	pub settlement_market_open_offset_min: i64,
	pub settlement_market_close_offset_min: i64,
}

impl Constants {
	fn new() -> Self {
		Self {
			time_slot_sec: read_env_or("TIME_SLOT_SEC", 900),
			execution_engine_offset_min: read_env_or("EXECUTION_ENGINE_OFFSET_MIN", -120),
			spot_market_open_offset_min: read_env_or("SPOT_MARKET_OPEN_OFFSET_MIN", -180),
			spot_market_close_offset_min: read_env_or("SPOT_MARKET_CLOSE_OFFSET_MIN", -60),
			flex_market_open_offset_min: read_env_or("FLEX_MARKET_OPEN_OFFSET_MIN", -15),
			flex_market_close_offset_min: read_env_or("FLEX_MARKET_CLOSE_OFFSET_MIN", 0),
			settlement_market_open_offset_min: read_env_or(
				"SETTLEMENT_MARKET_OPEN_OFFSET_MIN",
				-60,
			),
			settlement_market_close_offset_min: read_env_or(
				"SETTLEMENT_MARKET_CLOSE_OFFSET_MIN",
				-30,
			),
		}
	}
}

pub static GLOBAL_CONSTANTS: Lazy<Constants> = Lazy::new(Constants::new);
