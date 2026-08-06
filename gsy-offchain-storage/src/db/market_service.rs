use crate::db::DatabaseWrapper;
use anyhow::{bail, Result};
use futures::StreamExt;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;
use mongodb::{bson, Collection, IndexModel};
use primitives::db_api_schema::market::MarketSchema;
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
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<Vec<MarketSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = market_id {
            filter_params.insert("market_id", market_id);
        }
        if let Some(community_id) = community_id {
            filter_params.insert("community_id", community_id);
        }
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
        let mut result = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }

    #[tracing::instrument(name = "Saving market", skip(self, market), fields(market = ?market))]
    pub async fn upsert(&self, market: MarketSchema) -> Result<MarketSchema> {
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
