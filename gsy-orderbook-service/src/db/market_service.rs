use crate::db::DatabaseWrapper;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use anyhow::{bail, Result};
use futures::StreamExt;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::ops::Deref;


/// this function will call after connected to database
pub async fn init_markets(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.markets();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index).await?;
    Ok(())
}

#[repr(transparent)]
pub struct MarketService(pub Collection<MarketTopologySchema>);

impl MarketService {
    #[tracing::instrument(
        name = "Fetching market information from database", skip(self))]
    pub async fn filter(
        &self,
        market_id: String) -> Result<Vec<MarketTopologySchema>> {
        let mut cursor = self.0.find(
            doc! {"market_id": market_id.clone()}).await.unwrap();

        let mut result: Vec<MarketTopologySchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => {
                    result.push(document);
                }
                _ => {
                    break;
                }
            }
        }
        if result.len() > 1 {
            bail!("Found more than one market information for {}", market_id);
        }
        Ok(result)
    }

    #[tracing::instrument(
        name = "Fetching market information from database for a community", skip(self))]
    pub async fn get_community_market(
        &self,
        community_uuid: String, time_slot: u64) -> Result<Vec<MarketTopologySchema>> {
        let mut cursor = self.0.find(
            doc! {"community_uuid": community_uuid.clone(), "time_slot": time_slot as i32}
        ).await.unwrap();

        let mut result: Vec<MarketTopologySchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => {
                    result.push(document);
                }
                _ => {
                    break;
                }
            }
        }
        if result.len() > 1 {
            bail!("Found more than one market information for {} {}", community_uuid, time_slot);
        }
        Ok(result)
    }

    #[tracing::instrument(
        name = "Saving market to database",
        skip(self, market),
        fields(
        market = ?market
        )
    )]
    pub async fn insert(&self, market: MarketTopologySchema) -> Result<MarketTopologySchema> {
        self.check_if_market_exists(market.market_id.clone()).await?;
        match self.0.insert_one(market.clone()).await {
            Ok(_db_result) => Ok(market),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    async fn check_if_market_exists(&self, market_id: String) -> Result<bool> {
        match self.0.find(
            doc! {"market_id": market_id.clone()}).limit(1).await
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
        MarketService(db.collection("market"))
    }
}

impl Deref for MarketService {
    type Target = Collection<MarketTopologySchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
