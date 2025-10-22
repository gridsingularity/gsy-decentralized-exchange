use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, Cursor, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

/// this function will call after connected to database
pub async fn init_trades(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.trades();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index).await?;
    Ok(())
}

/// this struct is wrapper to `Collection<Trade>` should have function to help to manage order
#[repr(transparent)]
pub struct TradeService(pub Collection<TradeSchema>);

impl TradeService {
    #[tracing::instrument(name = "Fetching trades from database", skip(self))]
    pub async fn get_all_trades(&self) -> Result<Vec<TradeSchema>> {
        let mut cursor = self.0.find(doc! {}).await.unwrap();
        let mut result: Vec<TradeSchema> = Vec::new();
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
        Ok(result)
    }

    #[tracing::instrument(
        name = "Saving trades to database",
        skip(self, trade_schema),
        fields(
            trade_schema = ?trade_schema
        )
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
            match doc {
                Ok(document) => {
                    result.push(document);
                }
                _ => {
                    break;
                }
            }
        }
        Ok(result)
    }

    #[tracing::instrument(name = "Fetching trades by market id from database", skip(self))]
    pub async fn filter_trades(
        &self,
        market_id: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<TradeSchema>> {
        let mut filter_params = doc! {};
        if market_id.is_some() {
            filter_params.insert("market_id", market_id.unwrap());
        }
        if start_time.is_some() {
            filter_params.insert("time_slot", doc! {"$gte": start_time.unwrap()});
        }
        if end_time.is_some() {
            if start_time.is_some() {
                filter_params.insert(
                    "time_slot",
                    doc! {"$gte": start_time.unwrap(), "$lte": end_time.unwrap()},
                );
            } else {
                filter_params.insert("time_slot", doc! {"$lte": end_time.unwrap()});
            }
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
