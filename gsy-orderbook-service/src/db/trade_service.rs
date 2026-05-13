use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::trades::{
    ClearingResultSchema, MarketRoleSchema, TradeSchema,
};
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, Cursor, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

/// Trade indexes per D3.2 §5.3: `buyer`, `seller`, `market_id` and
/// `time_slot` accelerate per-asset / per-market / per-slot lookups.
pub async fn init_trades(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.trades();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"trade_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    for key in ["buyer", "seller", "market_id", "time_slot"] {
        controller
            .create_index(IndexModel::builder().keys(doc! {key: 1}).build())
            .await?;
    }
    Ok(())
}

pub async fn init_clearing_results(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.clearing_results();
    controller
        .create_index(IndexModel::builder().keys(doc! {"market_id": 1}).build())
        .await?;
    Ok(())
}

pub async fn init_market_roles(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.market_roles();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"role_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct TradeService(pub Collection<TradeSchema>);

impl TradeService {
    #[tracing::instrument(name = "Fetching trades from database", skip(self))]
    pub async fn get_all_trades(&self) -> Result<Vec<TradeSchema>> {
        let cursor = self.0.find(doc! {}).await?;
        self.create_vector_from_cursor(cursor).await
    }

    #[tracing::instrument(
        name = "Saving trades to database",
        skip(self, trade_schema),
        fields(trade_schema = ?trade_schema)
    )]
    pub async fn insert_trades(
        &self,
        trade_schema: Vec<TradeSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(trade_schema).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    async fn create_vector_from_cursor(
        &self,
        mut cursor: Cursor<TradeSchema>,
    ) -> Result<Vec<TradeSchema>> {
        let mut result: Vec<TradeSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            if let Ok(document) = doc {
                result.push(document);
            } else {
                break;
            }
        }
        Ok(result)
    }

    #[tracing::instrument(name = "Filter trades by time slot", skip(self))]
    pub async fn filter_trades(
        &self,
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<Vec<TradeSchema>> {
        let mut filter_params = doc! {};
        match (start_time, end_time) {
            (Some(start), Some(end)) => {
                filter_params.insert("time_slot", doc! {"$gte": start, "$lte": end});
            }
            (Some(start), None) => {
                filter_params.insert("time_slot", doc! {"$gte": start});
            }
            (None, Some(end)) => {
                filter_params.insert("time_slot", doc! {"$lte": end});
            }
            (None, None) => {}
        }

        match self.0.find(filter_params).await {
            Ok(cursor) => self.create_vector_from_cursor(cursor).await,
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

impl From<&DatabaseWrapper> for TradeService {
    fn from(db: &DatabaseWrapper) -> Self {
        TradeService(db.collection("trades"))
    }
}

impl Deref for TradeService {
    type Target = Collection<TradeSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct ClearingResultService(pub Collection<ClearingResultSchema>);

impl ClearingResultService {
    #[tracing::instrument(name = "Saving clearing result", skip(self, result))]
    pub async fn insert(&self, result: ClearingResultSchema) -> Result<ClearingResultSchema> {
        self.0.insert_one(result.clone()).await?;
        Ok(result)
    }

    #[tracing::instrument(name = "Fetching clearing result by market id", skip(self))]
    pub async fn get_by_market(&self, market_id: &str) -> Result<Vec<ClearingResultSchema>> {
        let mut cursor = self.0.find(doc! {"market_id": market_id}).await?;
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
}

impl From<&DatabaseWrapper> for ClearingResultService {
    fn from(db: &DatabaseWrapper) -> Self {
        ClearingResultService(db.collection("clearing_results"))
    }
}

impl Deref for ClearingResultService {
    type Target = Collection<ClearingResultSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct MarketRoleService(pub Collection<MarketRoleSchema>);

impl MarketRoleService {
    #[tracing::instrument(name = "Saving market role", skip(self, role))]
    pub async fn insert(&self, role: MarketRoleSchema) -> Result<MarketRoleSchema> {
        self.0.insert_one(role.clone()).await?;
        Ok(role)
    }

    #[tracing::instrument(name = "Fetching market role by name", skip(self))]
    pub async fn get_by_name(&self, role_name: &str) -> Result<Option<MarketRoleSchema>> {
        Ok(self.0.find_one(doc! {"role_name": role_name}).await?)
    }

    #[tracing::instrument(name = "Fetching all market roles", skip(self))]
    pub async fn get_all(&self) -> Result<Vec<MarketRoleSchema>> {
        let mut cursor = self.0.find(doc! {}).await?;
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
}

impl From<&DatabaseWrapper> for MarketRoleService {
    fn from(db: &DatabaseWrapper) -> Self {
        MarketRoleService(db.collection("market_roles"))
    }
}

impl Deref for MarketRoleService {
    type Target = Collection<MarketRoleSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
