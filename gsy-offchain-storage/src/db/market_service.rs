use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::{Result, bail};
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use mongodb::bson::doc;

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

    #[tracing::instrument(
        name = "Saving market to database",
        skip(self, market),
        fields(
        market = ?market
        )
    )]
    pub async fn insert(&self, market: MarketTopologySchema) -> Result<MarketTopologySchema> {
        self.check_if_market_exists(market.market_id.clone())
            .await?;
        self.0.insert_one(market.clone()).await?;
        Ok(market)
    }

    async fn check_if_market_exists(&self, market_id: String) -> Result<bool> {
        // NOTE: mirrors historical behavior — a successful lookup reports `true`
        // even with zero matches, so this never actually prevents duplicates.
        match self
            .0
            .query(doc! {"market_id": market_id.clone()}, |_| false)
            .await
        {
            Ok(_) => Ok(true),
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
