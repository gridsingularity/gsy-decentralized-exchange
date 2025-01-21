use crate::db::DatabaseWrapper;
use crate::db::schema::{TradeSchema};
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::{Collection, IndexModel};
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
        return Ok(result);
    }

    #[tracing::instrument(
        name = "Saving trades to database",
        skip(self, trade_schema),
        fields(
        trade_schema = ?trade_schema
        )
    )]
    pub async fn insert_trades(&self, trade_schema: Vec<TradeSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(trade_schema).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    // TODO: Filter trades by market id
    // #[tracing::instrument(name = "Fetching trades by market id from database", skip(self, id))]
    // pub async fn get_trades_by_market_id(&self, id: &Bson) -> Result<Option<TradeSchema>> {
    //     match self.0.find(doc! {"_id": id}, None).await {
    //         Ok(doc) => Ok(doc),
    //         Err(e) => {
    //             tracing::error!("Failed to execute query: {:?}", e);
    //             Err(anyhow::Error::from(e))
    //         }
    //     }
    // }
}

impl From<&DatabaseWrapper> for TradeService {
    fn from(db: &DatabaseWrapper) -> Self {
        TradeService(db.collection("orders"))
    }
}

impl Deref for TradeService {
    type Target = Collection<TradeSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
