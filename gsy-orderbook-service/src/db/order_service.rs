use crate::db::DatabaseWrapper;
use gsy_offchain_primitives::db_api_schema::orders::{OrderStatus, DbOrderSchema};
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::results::UpdateResult;
use mongodb::{bson, Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

/// this function will call after connected to database
pub async fn init_orders(db: &DatabaseWrapper) -> Result<()> {
    // create index in this block

    let controller = db.orders();
    let index: IndexModel = IndexModel::builder()
        .keys(doc! {"_id":1})
        .options(IndexOptions::builder().build())
        .build();
    controller.create_index(index).await?;
    Ok(())
}

/// this struct is wrapper to `Collection<Order>` should have function to help to manage order
#[repr(transparent)]
pub struct OrderService(pub Collection<DbOrderSchema>);

impl OrderService {
    #[tracing::instrument(name = "Fetching orders from database", skip(self))]
    pub async fn get_all_orders(&self) -> Result<Vec<DbOrderSchema>> {
        let mut cursor = self.0.find(doc! {}).await.unwrap();
        let mut result: Vec<DbOrderSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => {
                    result.push(document);
                }
                Err(err) => {
                    tracing::error!("Error while fetching orders: {}", err.to_string());
                    break;
                }
            }
        }
        Ok(result)
    }

    #[tracing::instrument(name = "Filter orders from database", skip(self))]
    pub async fn filter_orders(
            &self, market_id: Option<String>, start_time: Option<u32>,
            end_time: Option<u32>) -> Result<Vec<DbOrderSchema>> {
        let mut filter_params = doc! {};

        if market_id.is_some() {
            let market_id_str = market_id.unwrap();
            filter_params = doc! {"$or": [
                { "order.data.offer_component.market_id": market_id_str.clone() },
                { "order.data.bid_component.market_id": market_id_str.clone() }
            ]};
        }

        // TODO: Correct time_slot filtering based on nested offer / bid structs.
        if start_time.is_some() {
            filter_params.insert("time_slot", doc! {"$gte": start_time.unwrap()} ); }
        if end_time.is_some() {
            if start_time.is_some() {
                filter_params.insert("time_slot",
                                     doc! {"$gte": start_time.unwrap(), "$lte": end_time.unwrap()});
            }
            else {
                filter_params.insert("time_slot", doc! {"$lte": end_time.unwrap()});
            }
        }

        let mut cursor = self.0.find(filter_params).await.unwrap();
        let mut result: Vec<DbOrderSchema> = Vec::new();
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
        name = "Saving orders to database",
        skip(self, orders_schema),
        fields(
        orders_schema = ?orders_schema
        )
    )]
    pub async fn insert_orders(&self, orders_schema: Vec<DbOrderSchema>) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(orders_schema).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Fetching order by id from database", skip(self, id))]
    pub async fn get_order_by_id(&self, id: &Bson) -> Result<Option<DbOrderSchema>> {
        match self.0.find_one(doc! {"_id": id}).await {
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
    type Target = Collection<DbOrderSchema>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
