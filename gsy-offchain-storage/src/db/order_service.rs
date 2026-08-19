use crate::db::DatabaseWrapper;
use anyhow::Result;
use futures::StreamExt;
use mongodb::bson::{doc, Bson};
use mongodb::options::IndexOptions;
use mongodb::results::UpdateResult;
use mongodb::{bson, Collection, IndexModel};
use primitives::db_api_schema::orders::{DbOrderSchema, FlexibilityOrderSchema, OrderStatus};
use primitives::db_api_schema::tariff::TariffSchema;
use std::collections::HashMap;
use std::ops::Deref;

fn build_order_identifier_filter(id: &Bson) -> mongodb::bson::Document {
    match id {
        Bson::String(order_id) => doc! {
            "$or": [
                { "_id": Bson::String(order_id.clone()) },
                { "order_id": order_id.clone() }
            ]
        },
        _ => doc! { "_id": id.clone() },
    }
}

fn time_slot_bson(value: u64) -> Result<Bson> {
    Ok(Bson::Int64(i64::try_from(value)?))
}

/// Create the indexes required by the Order Book Storage. Per D3.2 section 5.4,
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
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Result<Vec<DbOrderSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = market_id {
            filter_params.insert("market_id", market_id);
        }
        match (start_time, end_time) {
            (Some(start), Some(end)) => {
                filter_params.insert(
                    "time_slot",
                    doc! {"$gte": time_slot_bson(start)?, "$lte": time_slot_bson(end)?},
                );
            }
            (Some(start), None) => {
                filter_params.insert("time_slot", doc! {"$gte": time_slot_bson(start)?});
            }
            (None, Some(end)) => {
                filter_params.insert("time_slot", doc! {"$lte": time_slot_bson(end)?});
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
        let mut upserted_ids = HashMap::new();

        for (index, order_schema) in orders_schema.into_iter().enumerate() {
            let order_id = order_schema.order_id.clone();
            let order_doc = bson::to_document(&order_schema)?;

            match self
                .0
                .update_one(
                    doc! { "order_id": order_id.clone() },
                    doc! { "$set": order_doc },
                )
                .upsert(true)
                .await
            {
                Ok(db_result) => {
                    upserted_ids.insert(
                        index,
                        db_result
                            .upserted_id
                            .unwrap_or_else(|| Bson::String(order_id.clone())),
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to execute query: {:?}", e);
                    return Err(anyhow::Error::from(e));
                }
            }
        }

        Ok(upserted_ids)
    }

    #[tracing::instrument(name = "Fetching order by id from database", skip(self, id))]
    pub async fn get_order_by_id(&self, id: &Bson) -> Result<Option<DbOrderSchema>> {
        let filter = build_order_identifier_filter(id);
        match self.0.find_one(filter).await {
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
                "status": bson::to_bson(&OrderStatus::Executed)?,
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

    #[tracing::instrument(name = "Update order status by id", skip(self, id, status))]
    pub async fn update_order_status_by_id(
        &self,
        id: &Bson,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        let filter = build_order_identifier_filter(id);
        match self
            .0
            .update_one(filter, doc! {"$set": {"status": bson::to_bson(&status)?}})
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
        now_time_slot: u64,
        status: OrderStatus,
    ) -> Result<UpdateResult> {
        match self
            .0
            .update_many(
                doc! {
                    "time_slot": {"$lt": time_slot_bson(now_time_slot)?},
                    "status": bson::to_bson(&OrderStatus::Open)?,
                },
                doc! {"$set": {"status": bson::to_bson(&status)?}},
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
    #[tracing::instrument(name = "Saving tariff", skip(self, tariff))]
    pub async fn insert(&self, tariff: TariffSchema) -> Result<TariffSchema> {
        self.0.insert_one(tariff.clone()).await?;
        Ok(tariff)
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
