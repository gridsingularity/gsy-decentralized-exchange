mod pay_as_bid;
mod pay_as_clear;

use crate::models::{BidOfferMatch, MatchingData};
use primitives::MatchingAlgorithm;

pub trait PayAsBid {
    type Output;

    fn pay_as_bid(&mut self) -> Vec<Self::Output>;
}

pub trait PayAsClear {
    type Output;

    fn pay_as_clear(&mut self) -> Vec<Self::Output>;
}

pub trait MatchOrders {
    fn match_orders(&self, matching_data: &mut MatchingData)
        -> Result<Vec<BidOfferMatch>, String>;
}

impl MatchOrders for MatchingAlgorithm {
    fn match_orders(
        &self,
        matching_data: &mut MatchingData,
    ) -> Result<Vec<BidOfferMatch>, String> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClearingPoint {
    traded_energy: u64,
    clearing_price: u64,
}
