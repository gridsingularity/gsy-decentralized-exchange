pub mod algorithms;
pub mod db_api_schema;

pub mod constants;
pub mod ewds;
pub mod log;
pub mod types;
pub mod utils;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MarketType {
    #[serde(rename = "spot")]
    Spot,
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "settlement")]
    Settlement,
}

impl MarketType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketType::Spot => "spot",
            MarketType::Flex => "flex",
            MarketType::Settlement => "settlement",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MatchingAlgorithm {
    #[serde(rename = "pay_as_bid")]
    PayAsBid,
    #[serde(rename = "pay_as_clear")]
    PayAsClear,
    #[serde(rename = "amm")]
    AMM,
}

impl MatchingAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchingAlgorithm::PayAsBid => "pay_as_bid",
            MatchingAlgorithm::PayAsClear => "pay_as_clear",
            MatchingAlgorithm::AMM => "amm",
        }
    }

    /// Runs the configured algorithm against one market order book.
    pub fn match_orders(
        &self,
        matching_data: &mut types::MatchingData,
    ) -> Result<Vec<types::BidOfferMatch>, String> {
        use algorithms::{PayAsBid, PayAsClear};

        matching_data.validate_market_slot()?;

        match self {
            MatchingAlgorithm::PayAsBid => Ok(matching_data.pay_as_bid()),
            MatchingAlgorithm::PayAsClear => Ok(matching_data.pay_as_clear()),
            MatchingAlgorithm::AMM => {
                Err("Matching algorithm 'amm' is not implemented".to_string())
            }
        }
    }
}

impl Default for MatchingAlgorithm {
    fn default() -> Self {
        Self::PayAsBid
    }
}

impl fmt::Display for MatchingAlgorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MatchingAlgorithm {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pay_as_bid" | "pay-as-bid" => Ok(Self::PayAsBid),
            "pay_as_clear" | "pay-as-clear" => Ok(Self::PayAsClear),
            "amm" => Ok(Self::AMM),
            _ => Err(format!(
                "Unsupported matching algorithm '{}'. Expected pay_as_bid, pay_as_clear, or amm",
                value
            )),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MarketTimeSeriesGranularity {
    #[serde(rename = "15min")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "1d")]
    OneDay,
}
