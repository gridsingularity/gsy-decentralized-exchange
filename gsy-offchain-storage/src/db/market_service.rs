use crate::db::DatabaseWrapper;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{bson, Collection, IndexModel};
use primitives::db_api_schema::market::MarketSchema;
use primitives::utils::normalize_community_id;
use std::ops::Deref;

pub async fn init_markets(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.markets();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"market_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"community_id": 1}).build())
        .await?;
    controller
        .create_index(IndexModel::builder().keys(doc! {"opening_time": 1}).build())
        .await?;
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"delivery_start_time": 1})
                .build(),
        )
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct MarketService(pub Collection<MarketSchema>);

impl MarketService {
    #[tracing::instrument(name = "Fetching markets", skip(self))]
    pub async fn filter(
        &self,
        market_id: Option<String>,
        community_id: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<MarketSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = market_id {
            filter_params.insert("market_id", market_id);
        }
        if let Some(community_id) = community_id {
            filter_params.insert("community_id", normalize_community_id(&community_id)?);
        }
        let mut opening_range = doc! {};
        if let Some(start) = start_time {
            opening_range.insert("$gte", bson::DateTime::from_chrono(start));
        }
        if let Some(end) = end_time {
            opening_range.insert("$lte", bson::DateTime::from_chrono(end));
        }
        if !opening_range.is_empty() {
            filter_params.insert("opening_time", opening_range);
        }

        Ok(self.0.find(filter_params).await?.try_collect().await?)
    }

    #[tracing::instrument(name = "Fetching markets by ids", skip(self))]
    pub async fn get_by_ids(&self, market_ids: Vec<String>) -> Result<Vec<MarketSchema>> {
        Ok(self.0.find(doc! {"market_id": {"$in": market_ids}}).await?.try_collect().await?)
    }

    /// IDs of the markets whose `delivery_start_time` lies in `[start_time, end_time)`.
    #[tracing::instrument(name = "Fetching market ids by delivery start", skip(self))]
    pub async fn market_ids_by_delivery_start(
        &self,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<String>> {
        let mut range = doc! {};
        if let Some(start) = start_time {
            range.insert("$gte", bson::DateTime::from_chrono(start));
        }
        if let Some(end) = end_time {
            range.insert("$lt", bson::DateTime::from_chrono(end));
        }

        let market_ids = self
            .0
            .distinct("market_id", doc! {"delivery_start_time": range})
            .await?;
        Ok(market_ids
            .into_iter()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect())
    }

    #[tracing::instrument(name = "Saving market", skip(self, market), fields(market = ?market))]
    pub async fn upsert(&self, mut market: MarketSchema) -> Result<MarketSchema> {
        market.community_id = normalize_community_id(&market.community_id)?;
        let market_doc = bson::to_document(&market)?;
        match self
            .0
            .update_one(
                doc! {"market_id": market.market_id.clone()},
                doc! {"$set": market_doc},
            )
            .upsert(true)
            .await
        {
            Ok(_) => Ok(market),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                bail!("Failed to upsert market: {:?}", e);
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
