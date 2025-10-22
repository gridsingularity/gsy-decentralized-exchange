pub mod algorithms;
pub mod db_api_schema;

pub mod node_to_api_schema;
pub mod types;
pub mod utils;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
