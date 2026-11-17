//! Market records of the order books. Each order book is matched with the
//! `matching_algorithm` of its market record, not with an engine-wide setting.

use crate::algorithms::MATCHABLE_ALGORITHMS;
use crate::models::MatchingData;
use anyhow::Result;
use async_trait::async_trait;
use primitives::db_api_schema::market::MarketSchema;
use primitives::offchain_storage::OffchainStorageClient;
use primitives::MatchingAlgorithm;
use std::collections::{BTreeSet, HashMap};
use tracing::warn;

/// Where the engine looks up market records.
#[async_trait]
pub trait MarketSource: Send + Sync {
    /// The record of `market_id`, or `None` if the market is unknown.
    async fn market(&self, market_id: &str) -> Result<Option<MarketSchema>>;
}

#[async_trait]
impl MarketSource for OffchainStorageClient {
    async fn market(&self, market_id: &str) -> Result<Option<MarketSchema>> {
        self.fetch_market(market_id).await
    }
}

/// Market records kept between matching cycles. `MarketController` never
/// changes a market after creating it, so a fetched record stays valid.
pub struct MarketCache<S> {
    source: S,
    known: HashMap<String, MarketSchema>,
}

impl<S: MarketSource> MarketCache<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            known: HashMap::new(),
        }
    }

    /// Records of `market_ids`, keyed by market ID. Uncached markets are
    /// fetched; unknown markets and failed lookups are logged and left out,
    /// so they are fetched again next cycle. Markets not in `market_ids` are
    /// evicted.
    pub async fn resolve(
        &mut self,
        market_ids: &BTreeSet<String>,
    ) -> &HashMap<String, MarketSchema> {
        self.known
            .retain(|market_id, _| market_ids.contains(market_id));
        for market_id in market_ids {
            if self.known.contains_key(market_id) {
                continue;
            }
            match self.source.market(market_id).await {
                Ok(Some(market)) => {
                    self.known.insert(market_id.clone(), market);
                }
                Ok(None) => warn!(
                    "Market {} is not in off-chain storage; skipping its order books until it is",
                    market_id
                ),
                Err(error) => warn!(
                    "Failed to look up market {}: {:?}; skipping its order books this cycle",
                    market_id, error
                ),
            }
        }
        &self.known
    }
}

/// Groups the order books by the matching algorithm of their market record.
/// Books of markets without a record, and of markets whose algorithm the
/// engine does not implement (`amm`), are left out; their orders stay open.
pub fn group_by_matching_algorithm(
    order_books: Vec<MatchingData>,
    markets: &HashMap<String, MarketSchema>,
) -> HashMap<MatchingAlgorithm, Vec<MatchingData>> {
    let mut groups: HashMap<MatchingAlgorithm, Vec<MatchingData>> = HashMap::new();
    for order_book in order_books {
        // `MarketCache::resolve` has already logged unknown markets.
        let Some(market) = markets.get(order_book.market_id()) else {
            continue;
        };
        if !MATCHABLE_ALGORITHMS.contains(&market.matching_algorithm) {
            warn!(
                "Skipping market {} timeslot {}: matching algorithm '{}' is not implemented",
                order_book.market_id(),
                order_book.time_slot(),
                market.matching_algorithm
            );
            continue;
        }
        groups
            .entry(market.matching_algorithm)
            .or_default()
            .push(order_book);
    }
    groups
}
