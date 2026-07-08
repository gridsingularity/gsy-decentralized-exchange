pub mod algorithms;
pub mod db_api_schema;

pub mod constants;
pub mod types;
pub mod utils;
pub mod log;

use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum MarketType {
    #[serde(rename = "spot")]
    Spot,
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "settlement")]
    Settlement,
}
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum MatchingAlgorithm {
    #[serde(rename = "pay_as_bid")]
    PayAsBid,
    #[serde(rename = "pay_as_clear")]
    PayAsClear,
    #[serde(rename = "amm")]
    AMM,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub enum MarketTimeSeriesGranularity {
    #[serde(rename = "15min")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "1d")]
    OneDay,
}