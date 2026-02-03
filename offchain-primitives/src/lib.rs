pub mod algorithms;
pub mod db_api_schema;
pub mod types;
pub mod utils;
pub mod constants;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketType {
	Spot,
	Flexibility,
	Settlement,
}

impl MarketType {
	pub fn as_str(&self) -> &'static str {
		match self {
			MarketType::Spot => "Spot",
			MarketType::Flexibility => "Flexibility",
			MarketType::Settlement => "Settlement",
		}
	}
}
