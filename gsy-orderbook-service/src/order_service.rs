use crate::db::DatabaseWrapper;
use crate::schema::{OrderSchema, OrderStatus};
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::results::UpdateResult;
use mongodb::{bson, Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

/// this function will call after connected to database
pub async fn init(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.orders();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index, None).await?;
    Ok(())
}

/// this struct is wrapper to `Collection<Order>` should have function to help to manage order
#[repr(transparent)]
pub struct OrderService(pub Collection<OrderSchema>);

impl OrderService {
    #[tracing::instrument(name = "Fetching orders from database", skip(self))]
    pub async fn get_all_orders(&self) -> Result<Vec<OrderSchema>> {
        let mut cursor = self.0.find(doc! {}, None).await.unwrap();
        let mut result: Vec<OrderSchema> = Vec::new();
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
        name = "Saving orders to database",
        skip(self, orders_schema),
        fields(
        orders_schema = ?orders_schema
        )
    )]
    pub async fn insert_orders(&self, orders_schema: Vec<OrderSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(orders_schema, None).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Fetching order by id from database", skip(self, id))]
    pub async fn get_order_by_id(&self, id: &Bson) -> Result<Option<OrderSchema>> {
        match self.0.find_one(doc! {"_id": id}, None).await {
            Ok(doc) => Ok(doc),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Update order status by id", skip(self, id, status))]
    pub async fn update_order_status_by_id(
        &self,
        id: &Bson,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        match self
            .0
            .update_one(
                doc! {
                    "_id": id
                },
                doc! {
                    "$set": {"status": bson::to_bson(&status).unwrap()}
                },
                None,
            )
            .await
        {
            Ok(doc) => Ok(doc),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Update expired orders", skip(self, now_time_slot))]
    pub async fn update_expired_orders(
        &self,
        now_time_slot: u64,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        match self
            .0
            .update_many(
                doc! {
                    "order.data.time_slot": { "$lt": bson::to_bson(&now_time_slot).unwrap()},
                    "status": bson::to_bson(&OrderStatus::Open).unwrap()
                },
                doc! {
                    "$set": { "status": bson::to_bson(&status).unwrap()},
                },
                None,
            )
            .await
        {
            Ok(doc) => Ok(doc),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }
}

impl From<&DatabaseWrapper> for OrderService {
    fn from(db: &DatabaseWrapper) -> Self {
        OrderService(db.collection("orders"))
    }
}

impl Deref for OrderService {
    type Target = Collection<OrderSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
