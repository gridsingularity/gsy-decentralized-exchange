use crate::db::DatabaseWrapper;
use anyhow::{bail, Result};
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::market::MarketSchema;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
use std::ops::Deref;

pub async fn init_markets(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.markets();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"market_id": 1, "opening_time": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"community_id": 1}).build())
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct MarketService(pub Collection<MarketSchema>);

impl MarketService {
    #[tracing::instrument(name = "Fetching market by id", skip(self))]
    pub async fn filter(&self, market_id: String) -> Result<Vec<MarketSchema>> {
        let mut cursor = self.0.find(doc! {"market_id": market_id.clone()}).await?;
        let mut result: Vec<MarketSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }

    /// Return markets for a community filtered by an opening-time window.
    /// `start_time` / `end_time` are matched against `opening_time` as
    /// ISO 8601 strings (lexicographic ordering matches chronological
    /// ordering for these timestamps).
    #[tracing::instrument(name = "Fetching markets for a community", skip(self))]
    pub async fn get_community_market(
        &self,
        community_id: String,
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<Vec<MarketSchema>> {
        let mut filter_params = doc! {"community_id": community_id};
        match (start_time, end_time) {
            (Some(start), Some(end)) => {
                filter_params.insert("opening_time", doc! {"$gte": start, "$lte": end});
            }
            (Some(start), None) => {
                filter_params.insert("opening_time", doc! {"$gte": start});
            }
            (None, Some(end)) => {
                filter_params.insert("opening_time", doc! {"$lte": end});
            }
            (None, None) => {}
        }

        let mut cursor = self.0.find(filter_params).await?;
        let mut result: Vec<MarketSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }

    #[tracing::instrument(
        name = "Saving market to database",
        skip(self, market),
        fields(market = ?market)
    )]
    pub async fn insert(&self, market: MarketSchema) -> Result<MarketSchema> {
        match self.0.insert_one(market.clone()).await {
            Ok(_) => Ok(market),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                bail!("Failed to insert market: {:?}", e);
            }
        }
    }
}

impl From<&DatabaseWrapper> for MarketService {
    fn from(db: &DatabaseWrapper) -> Self {
        MarketService(db.collection("markets"))
    }
}

impl Deref for MarketService {
    type Target = Collection<MarketSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
