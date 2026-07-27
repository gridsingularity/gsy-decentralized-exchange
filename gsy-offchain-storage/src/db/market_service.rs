use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::{Result, bail};
use gsy_offchain_primitives::db_api_schema::market::{CommunitySummary, MarketTopologySchema};
use mongodb::bson::doc;
use std::collections::HashMap;

pub struct MarketService(pub(crate) Coll<MarketTopologySchema>);

impl MarketService {
    #[tracing::instrument(name = "Fetching market information from database", skip(self))]
    pub async fn filter(&self, market_id: String) -> Result<Vec<MarketTopologySchema>> {
        let result = self
            .0
            .query(doc! {"market_id": market_id.clone()}, |market| {
                market.market_id == market_id
            })
            .await?;
        if result.len() > 1 {
            bail!("Found more than one market information for {}", market_id);
        }
        Ok(result)
    }

    #[tracing::instrument(
        name = "Fetching market information from database for a community",
        skip(self)
    )]
    pub async fn get_community_market(
        &self,
        community_name: String,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<MarketTopologySchema>> {
        let mut filter_params = doc! {};
        filter_params.insert("community_name", community_name.clone());
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |market| {
                market.community_name == community_name
                    && in_time_window(market.time_slot as u64, start_time, end_time)
            })
            .await
    }

    #[tracing::instrument(
        name = "Fetching all markets within a delivery time window",
        skip(self)
    )]
    pub async fn get_markets_in_time_range(
        &self,
        start_time: u32,
        end_time: u32,
    ) -> Result<Vec<MarketTopologySchema>> {
        self.0
            .query(
                doc! {"time_slot": {"$gte": start_time, "$lte": end_time}},
                |market| market.time_slot >= start_time && market.time_slot <= end_time,
            )
            .await
    }

    #[tracing::instrument(name = "Fetching all markets from database", skip(self))]
    pub async fn all_markets(&self) -> Result<Vec<MarketTopologySchema>> {
        self.0.all().await
    }

    #[tracing::instrument(name = "Listing communities from database", skip(self))]
    pub async fn list_communities(&self) -> Result<Vec<CommunitySummary>> {
        let markets = self.all_markets().await?;

        let mut groups: HashMap<String, CommunitySummary> = HashMap::new();
        // Track the time_slot of the market that currently supplies each
        // group's community_uuid, so we can keep the latest one.
        let mut uuid_slot: HashMap<String, u32> = HashMap::new();

        for market in markets {
            match groups.get_mut(&market.community_name) {
                Some(summary) => {
                    summary.market_count += 1;
                    summary.earliest_slot = summary.earliest_slot.min(market.time_slot);
                    summary.latest_slot = summary.latest_slot.max(market.time_slot);
                    let current_uuid_slot = uuid_slot.get(&market.community_name).copied().unwrap_or(0);
                    if market.time_slot >= current_uuid_slot {
                        summary.community_uuid = market.community_uuid.clone();
                        uuid_slot.insert(market.community_name.clone(), market.time_slot);
                    }
                }
                None => {
                    uuid_slot.insert(market.community_name.clone(), market.time_slot);
                    groups.insert(
                        market.community_name.clone(),
                        CommunitySummary {
                            community_name: market.community_name,
                            community_uuid: market.community_uuid,
                            market_count: 1,
                            earliest_slot: market.time_slot,
                            latest_slot: market.time_slot,
                        },
                    );
                }
            }
        }

        let mut communities: Vec<CommunitySummary> = groups.into_values().collect();
        communities.sort_by(|a, b| a.community_name.cmp(&b.community_name));
        Ok(communities)
    }

    #[tracing::instrument(
        name = "Saving market to database",
        skip(self, market),
        fields(
        market = ?market
        )
    )]
    pub async fn insert(&self, market: MarketTopologySchema) -> Result<MarketTopologySchema> {
        if self
            .check_if_market_exists(market.market_id.clone())
            .await?
        {
            bail!(
                "Market with id {} already exists; refusing to insert a duplicate",
                market.market_id
            );
        }
        self.0.insert_one(market.clone()).await?;
        Ok(market)
    }

    /// Returns whether a market with `market_id` already exists. Uses a bounded
    /// `find_one` probe (not an unbounded drain) and reports existence from
    /// whether a document was actually matched.
    async fn check_if_market_exists(&self, market_id: String) -> Result<bool> {
        match self
            .0
            .find_one(doc! {"market_id": market_id.clone()}, |market| {
                market.market_id == market_id
            })
            .await
        {
            Ok(existing) => Ok(existing.is_some()),
            Err(_) => {
                bail!("Failed find market with id: {:?}", market_id);
            }
        }
    }
}

impl From<&DatabaseWrapper> for MarketService {
    fn from(db: &DatabaseWrapper) -> Self {
        MarketService(db.coll("market", |store| store.markets.clone()))
    }
}
