use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, FlexibilityOrderSchema, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::tariff::TariffSchema;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::results::UpdateResult;
use mongodb::{bson, Collection, IndexModel};
use std::collections::HashMap;
use std::ops::Deref;

/// Create the indexes required by the Order Book Storage. Per D3.2 §5.4,
/// `created_by`, `market_id` and `time_slot` are indexed to accelerate
/// queries that filter bids/offers for an asset, market or time slot.
pub async fn init_orders(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.orders();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"order_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    for key in ["created_by", "market_id", "time_slot"] {
        controller
            .create_index(IndexModel::builder().keys(doc! {key: 1}).build())
            .await?;
    }
    Ok(())
}

pub async fn init_flexibility_orders(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.flexibility_orders();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"order_id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

pub async fn init_tariffs(db: &DatabaseWrapper) -> Result<()> {
    let controller = db.tariffs();
    controller
        .create_index(
            IndexModel::builder()
                .keys(doc! {"tariff_name": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;
    Ok(())
}

#[repr(transparent)]
pub struct OrderService(pub Collection<DbOrderSchema>);

impl OrderService {
    #[tracing::instrument(name = "Fetching orders from database", skip(self))]
    pub async fn get_all_orders(&self) -> Result<Vec<DbOrderSchema>> {
        let mut cursor = self.0.find(doc! {}).await?;
        let mut result: Vec<DbOrderSchema> = Vec::new();
        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => result.push(document),
                Err(err) => {
                    tracing::error!("Error while fetching orders: {}", err);
                    break;
                }
            }
        }
        Ok(result)
    }

    #[tracing::instrument(name = "Filter orders from database", skip(self))]
    pub async fn filter_orders(
        &self,
        market_id: Option<String>,
        start_time: Option<String>,
        end_time: Option<String>,
    ) -> Result<Vec<DbOrderSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = market_id {
            filter_params.insert("market_id", market_id);
        }
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

        let mut cursor = self.0.find(filter_params).await?;
        let mut result: Vec<DbOrderSchema> = Vec::new();
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
        name = "Saving orders to database",
        skip(self, orders_schema),
        fields(orders_schema = ?orders_schema)
    )]
    pub async fn insert_orders(
        &self,
        orders_schema: Vec<DbOrderSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        match self.0.insert_many(orders_schema).await {
            Ok(db_result) => Ok(db_result.inserted_ids),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Fetching order by id from database", skip(self))]
    pub async fn get_order_by_id(&self, order_id: &str) -> Result<Option<DbOrderSchema>> {
        match self.0.find_one(doc! {"order_id": order_id}).await {
            Ok(doc) => Ok(doc),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    pub async fn update_order_by_area_market_id(
        &self,
        area_uuid: String,
        market_id: String,
    ) -> Result<bool> {
        let filter = doc! {
            "area_uuid": area_uuid,
            "market_id": market_id,
        };
        let update = doc! {
            "$set": {
                "order_status": bson::to_bson(&OrderStatus::Executed)?,
            }
        };
        match self.0.update_many(filter, update).await {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                Err(anyhow::Error::from(e))
            }
        }
    }

    #[tracing::instrument(name = "Update order status by id", skip(self))]
    pub async fn update_order_status_by_id(
        &self,
        order_id: &str,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        match self
            .0
            .update_one(
                doc! {"order_id": order_id},
                doc! {"$set": {"order_status": bson::to_bson(&status)?}},
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

    #[tracing::instrument(name = "Update expired orders", skip(self))]
    pub async fn update_expired_orders(
        &self,
        now_time_slot: String,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        match self
            .0
            .update_many(
                doc! {
                    "time_slot": {"$lt": now_time_slot},
                    "order_status": bson::to_bson(&OrderStatus::Open)?,
                },
                doc! {"$set": {"order_status": bson::to_bson(&status)?}},
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

#[repr(transparent)]
pub struct FlexibilityOrderService(pub Collection<FlexibilityOrderSchema>);

impl FlexibilityOrderService {
    #[tracing::instrument(name = "Saving flexibility orders", skip(self, orders))]
    pub async fn insert_orders(
        &self,
        orders: Vec<FlexibilityOrderSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        Ok(self.0.insert_many(orders).await?.inserted_ids)
    }

    #[tracing::instrument(name = "Fetching all flexibility orders", skip(self))]
    pub async fn get_all_orders(&self) -> Result<Vec<FlexibilityOrderSchema>> {
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

impl From<&DatabaseWrapper> for FlexibilityOrderService {
    fn from(db: &DatabaseWrapper) -> Self {
        FlexibilityOrderService(db.collection("flexibility_orders"))
    }
}

impl Deref for FlexibilityOrderService {
    type Target = Collection<FlexibilityOrderSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(transparent)]
pub struct TariffService(pub Collection<TariffSchema>);

impl TariffService {
    #[tracing::instrument(name = "Inserting tariff", skip(self, tariff))]
    pub async fn insert(&self, tariff: TariffSchema) -> Result<TariffSchema> {
        self.0.insert_one(tariff.clone()).await?;
        Ok(tariff)
    }

    #[tracing::instrument(name = "Fetching tariff by name", skip(self))]
    pub async fn get_by_name(&self, tariff_name: &str) -> Result<Option<TariffSchema>> {
        Ok(self.0.find_one(doc! {"tariff_name": tariff_name}).await?)
    }

    #[tracing::instrument(name = "Fetching all tariffs", skip(self))]
    pub async fn get_all(&self) -> Result<Vec<TariffSchema>> {
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

impl From<&DatabaseWrapper> for TariffService {
    fn from(db: &DatabaseWrapper) -> Self {
        TariffService(db.collection("tariffs"))
    }
}

impl Deref for TariffService {
    type Target = Collection<TariffSchema>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
