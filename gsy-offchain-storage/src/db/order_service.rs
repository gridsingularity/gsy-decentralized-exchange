use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, UpdateSummary, in_time_window, time_window_bounds};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, OrderStatus};
use mongodb::bson::{Bson, doc};
use mongodb::bson;
use std::collections::HashMap;

/// this struct is wrapper to `Collection<Order>` should have function to help to manage order
pub struct OrderService(pub(crate) Coll<DbOrderSchema>);

impl From<&DatabaseWrapper> for OrderService {
    fn from(db: &DatabaseWrapper) -> Self {
        OrderService(db.coll("orders", |store| store.orders.clone()))
    }
}

fn order_market_id(order: &Order) -> &str {
    match order {
        Order::Bid(bid) => &bid.bid_component.market_id,
        Order::Offer(offer) => &offer.offer_component.market_id,
    }
}

fn order_time_slot(order: &Order) -> u64 {
    match order {
        Order::Bid(bid) => bid.bid_component.time_slot,
        Order::Offer(offer) => offer.offer_component.time_slot,
    }
}

impl OrderService {
    #[tracing::instrument(name = "Fetching orders from database", skip(self))]
    pub async fn get_all_orders(&self) -> Result<Vec<DbOrderSchema>> {
        self.0.all().await
    }

    #[tracing::instrument(name = "Filter orders from database", skip(self))]
    pub async fn filter_orders(
        &self,
        market_id: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<DbOrderSchema>> {
        // An order is either a Bid or an Offer, so its market_id / time_slot
        // live under exactly one of the two nested component paths. Each filter
        // is expressed as an `$or` over both paths; when both are present they
        // are combined with `$and` (two `$or` keys cannot coexist in one doc).
        let mut clauses: Vec<mongodb::bson::Document> = Vec::new();

        if let Some(market_id) = &market_id {
            clauses.push(doc! {"$or": [
                { "order.data.offer_component.market_id": market_id.clone() },
                { "order.data.bid_component.market_id": market_id.clone() }
            ]});
        }

        if let Some(bounds) = time_window_bounds(start_time, end_time) {
            clauses.push(doc! {"$or": [
                { "order.data.bid_component.time_slot": bounds.clone() },
                { "order.data.offer_component.time_slot": bounds }
            ]});
        }

        let filter_params = match clauses.len() {
            0 => doc! {},
            1 => clauses.pop().unwrap(),
            _ => doc! {"$and": clauses},
        };

        self.0
            .query(filter_params, |order| {
                market_id
                    .as_ref()
                    .is_none_or(|market_id| order_market_id(&order.order) == market_id)
                    && in_time_window(order_time_slot(&order.order), start_time, end_time)
            })
            .await
    }

    #[tracing::instrument(
        name = "Saving orders to database",
        skip(self, orders_schema),
        fields(
        orders_schema = ?orders_schema
        )
    )]
    pub async fn insert_orders(
        &self,
        orders_schema: Vec<DbOrderSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        self.0
            .insert_many(orders_schema, |order| Bson::String(order._id.clone()))
            .await
    }

    #[tracing::instrument(name = "Fetching order by id from database", skip(self, id))]
    pub async fn get_order_by_id(&self, id: &Bson) -> Result<Option<DbOrderSchema>> {
        self.0
            .find_one(doc! {"_id": id}, |order| {
                matches!(id, Bson::String(id) if &order._id == id)
            })
            .await
    }

    pub async fn update_order_by_area_market_id(
        &self,
        area_uuid: String,
        market_id: String,
    ) -> Result<bool> {
        // DbOrderSchema has no top-level `area_uuid`/`market_id` fields, so this
        // filter matches no documents on either backend (predicate mirrors Mongo).
        let filter = doc! {
            "area_uuid": area_uuid,
            "market_id": market_id
        };

        let update = doc! {
            "$set": {
                "status": bson::to_bson(&OrderStatus::Executed)?,
            }
        };

        self.0
            .update_many(filter, update, |_| false, |_| false)
            .await?;
        Ok(true)
    }

    #[tracing::instrument(name = "Update order status by id", skip(self, id, status))]
    pub async fn update_order_status_by_id(
        &self,
        id: &Bson,
        status: OrderStatus,
    ) -> Result<UpdateSummary> {
        self.0
            .update_one(
                doc! {
                    "_id": id
                },
                doc! {
                    "$set": {"status": bson::to_bson(&status).unwrap()}
                },
                |order| matches!(id, Bson::String(id) if &order._id == id),
                |order| {
                    let modified = order.status != status;
                    order.status = status.clone();
                    modified
                },
            )
            .await
    }

    #[tracing::instrument(name = "Update expired orders", skip(self, now_time_slot))]
    pub async fn update_expired_orders(
        &self,
        now_time_slot: u64,
        status: OrderStatus,
    ) -> Result<UpdateSummary> {
        // The order's time_slot lives at `order.data.{bid,offer}_component.time_slot`,
        // so the `order.data.time_slot` filter matches no documents on either
        // backend (predicate mirrors Mongo).
        self.0
            .update_many(
                doc! {
                    "order.data.time_slot": { "$lt": bson::to_bson(&now_time_slot).unwrap()},
                    "status": bson::to_bson(&OrderStatus::Open).unwrap()
                },
                doc! {
                    "$set": { "status": bson::to_bson(&status).unwrap()},
                },
                |_| false,
                |_| false,
            )
            .await
    }
}
