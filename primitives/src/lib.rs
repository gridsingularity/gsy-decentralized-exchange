pub mod db_api_schema;

pub mod constants;
pub mod ewds;
pub mod log;
pub mod matching;
pub mod offchain_storage;
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

    /// Value of `MarketController.MarketType`.
    pub fn to_evm(&self) -> u8 {
        match self {
            MarketType::Spot => 0,
            MarketType::Flex => 1,
            MarketType::Settlement => 2,
        }
    }

    pub fn from_evm(value: u8) -> Option<Self> {
        match value {
            0 => Some(MarketType::Spot),
            1 => Some(MarketType::Flex),
            2 => Some(MarketType::Settlement),
            _ => None,
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

    /// Value of `MarketController.MatchingAlgorithm`.
    pub fn to_evm(&self) -> u8 {
        match self {
            MatchingAlgorithm::PayAsBid => 0,
            MatchingAlgorithm::PayAsClear => 1,
            MatchingAlgorithm::AMM => 2,
        }
    }

    pub fn from_evm(value: u8) -> Option<Self> {
        match value {
            0 => Some(MatchingAlgorithm::PayAsBid),
            1 => Some(MatchingAlgorithm::PayAsClear),
            2 => Some(MatchingAlgorithm::AMM),
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_type_round_trips_through_evm_value() {
        for (market_type, value) in [
            (MarketType::Spot, 0),
            (MarketType::Flex, 1),
            (MarketType::Settlement, 2),
        ] {
            assert_eq!(market_type.to_evm(), value);
            assert_eq!(MarketType::from_evm(value), Some(market_type));
        }
        assert_eq!(MarketType::from_evm(3), None);
    }

    #[test]
    fn matching_algorithm_round_trips_through_evm_value() {
        for (algorithm, value) in [
            (MatchingAlgorithm::PayAsBid, 0),
            (MatchingAlgorithm::PayAsClear, 1),
            (MatchingAlgorithm::AMM, 2),
        ] {
            assert_eq!(algorithm.to_evm(), value);
            assert_eq!(MatchingAlgorithm::from_evm(value), Some(algorithm));
        }
        assert_eq!(MatchingAlgorithm::from_evm(3), None);
    }
}
