use anyhow::{anyhow, Result};
use async_trait::async_trait;
use gsy_matching_engine::markets::{group_by_matching_algorithm, MarketCache, MarketSource};
use gsy_matching_engine::models::{MatchingData, Order};
use primitives::db_api_schema::market::{MarketSchema, MarketType};
use primitives::db_api_schema::orders::{OrderEnum, OrderStatus};
use primitives::MatchingAlgorithm;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

fn market(market_id: &str, matching_algorithm: MatchingAlgorithm) -> MarketSchema {
    MarketSchema {
        market_id: market_id.to_string(),
        community_id: "00000000-0000-0000-0000-000000000001".to_string(),
        opening_time: "0".to_string(),
        closing_time: "0".to_string(),
        delivery_start_time: "0".to_string(),
        delivery_end_time: "0".to_string(),
        market_type: MarketType::Spot,
        matching_algorithm,
        created_at: "0".to_string(),
    }
}

#[derive(Clone)]
enum Answer {
    Found(MarketSchema),
    Failure,
}

/// Answers lookups from a table that a test can change between cycles and
/// records every lookup. Markets missing from the table are unknown. Clones
/// share their state, so a test keeps a handle on the source it hands over.
#[derive(Clone, Default)]
struct FakeSource {
    answers: Arc<Mutex<HashMap<String, Answer>>>,
    lookups: Arc<Mutex<Vec<String>>>,
}

impl FakeSource {
    fn found(&self, market_id: &str, matching_algorithm: MatchingAlgorithm) {
        self.answers.lock().unwrap().insert(
            market_id.to_string(),
            Answer::Found(market(market_id, matching_algorithm)),
        );
    }

    fn failing(&self, market_id: &str) {
        self.answers
            .lock()
            .unwrap()
            .insert(market_id.to_string(), Answer::Failure);
    }

    fn lookups(&self) -> Vec<String> {
        self.lookups.lock().unwrap().clone()
    }
}

#[async_trait]
impl MarketSource for FakeSource {
    async fn market(&self, market_id: &str) -> Result<Option<MarketSchema>> {
        self.lookups.lock().unwrap().push(market_id.to_string());
        let answer = self.answers.lock().unwrap().get(market_id).cloned();
        match answer {
            Some(Answer::Found(market)) => Ok(Some(market)),
            Some(Answer::Failure) => Err(anyhow!("storage unavailable")),
            None => Ok(None),
        }
    }
}

fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn markets(entries: &[(&str, MatchingAlgorithm)]) -> HashMap<String, MarketSchema> {
    entries
        .iter()
        .map(|(market_id, algorithm)| (market_id.to_string(), market(market_id, *algorithm)))
        .collect()
}

#[tokio::test]
async fn resolved_markets_are_looked_up_once() {
    let source = FakeSource::default();
    source.found("a", MatchingAlgorithm::PayAsBid);
    source.found("b", MatchingAlgorithm::PayAsClear);
    let mut cache = MarketCache::new(source.clone());

    for _ in 0..2 {
        assert_eq!(
            cache.resolve(&ids(&["a", "b"])).await,
            &markets(&[
                ("a", MatchingAlgorithm::PayAsBid),
                ("b", MatchingAlgorithm::PayAsClear),
            ])
        );
    }
    assert_eq!(source.lookups(), ["a", "b"]);
}

#[tokio::test]
async fn unknown_and_failed_markets_are_retried_next_cycle() {
    let source = FakeSource::default();
    source.failing("failing");
    let mut cache = MarketCache::new(source.clone());

    assert!(cache
        .resolve(&ids(&["failing", "missing"]))
        .await
        .is_empty());

    source.found("failing", MatchingAlgorithm::PayAsClear);
    source.found("missing", MatchingAlgorithm::PayAsBid);
    assert_eq!(
        cache.resolve(&ids(&["failing", "missing"])).await,
        &markets(&[
            ("failing", MatchingAlgorithm::PayAsClear),
            ("missing", MatchingAlgorithm::PayAsBid),
        ])
    );
    assert_eq!(
        source.lookups(),
        ["failing", "missing", "failing", "missing"]
    );
}

#[tokio::test]
async fn markets_without_order_books_are_evicted() {
    let source = FakeSource::default();
    source.found("a", MatchingAlgorithm::PayAsBid);
    source.found("b", MatchingAlgorithm::PayAsClear);
    let mut cache = MarketCache::new(source.clone());

    cache.resolve(&ids(&["a", "b"])).await;
    assert_eq!(
        cache.resolve(&ids(&["a"])).await,
        &markets(&[("a", MatchingAlgorithm::PayAsBid)])
    );
    cache.resolve(&ids(&["a", "b"])).await;

    assert_eq!(source.lookups(), ["a", "b", "b"]);
}

fn order(market_id: &str, order_type: OrderEnum) -> Order {
    let label = format!("{market_id}-{order_type:?}");
    Order {
        order_id: label.clone(),
        order_type,
        status: OrderStatus::Submitted,
        area_uuid: format!("{label}-area"),
        market_id: market_id.to_string(),
        time_slot: 100,
        creation_time: 1,
        energy: 1,
        energy_rate: 1,
        created_by: format!("{label}-actor"),
        requirements: None,
        attributes: None,
    }
}

fn order_book(market_id: &str) -> MatchingData {
    MatchingData::new(
        market_id.to_string(),
        100,
        vec![order(market_id, OrderEnum::Bid)],
        vec![order(market_id, OrderEnum::Offer)],
    )
    .unwrap()
}

#[test]
fn order_books_are_grouped_by_the_algorithm_of_their_market() {
    let known = markets(&[
        ("amm", MatchingAlgorithm::AMM),
        ("pab-1", MatchingAlgorithm::PayAsBid),
        ("pab-2", MatchingAlgorithm::PayAsBid),
        ("pac", MatchingAlgorithm::PayAsClear),
    ]);
    let order_books = ["amm", "pab-1", "pac", "pab-2", "unknown"]
        .into_iter()
        .map(order_book)
        .collect();

    let groups = group_by_matching_algorithm(order_books, &known)
        .into_iter()
        .map(|(algorithm, order_books)| {
            let market_ids = order_books
                .iter()
                .map(|order_book| order_book.market_id().to_string())
                .collect::<Vec<_>>();
            (algorithm, market_ids)
        })
        .collect::<HashMap<_, _>>();

    // The books of the unknown and the amm market are left out.
    assert_eq!(
        groups,
        HashMap::from([
            (
                MatchingAlgorithm::PayAsBid,
                vec!["pab-1".to_string(), "pab-2".to_string()]
            ),
            (MatchingAlgorithm::PayAsClear, vec!["pac".to_string()]),
        ])
    );
}
