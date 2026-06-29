pub mod algorithms;
pub mod db_api_schema;

pub mod constants;
pub mod types;
pub mod utils;

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
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
