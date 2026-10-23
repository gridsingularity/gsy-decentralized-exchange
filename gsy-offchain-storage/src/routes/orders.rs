use crate::db::DbRef;
use actix_web::{HttpResponse, Responder, web::Json, web::Query};
use anyhow::{Error, Result};
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use gsy_offchain_primitives::node_to_api_schema::insert_order::convert_gsy_node_order_schema_to_db_schema;
use serde::Deserialize;
use std::time::{Duration, Instant};

/// Above this duration an order post is logged at `warn`: the orderbook worker gives up (and
/// panics) after 2 s, so a slow post is worth seeing before it reaches that deadline.
const SLOW_ORDER_POST: Duration = Duration::from_secs(1);

/// Store a batch of orders and answer with the `_id`s of the newly stored ones. Idempotent: an
/// order already stored is skipped with its status kept, and the post still answers 200, so the
/// orderbook worker takes its success path for re-posted orders.
/// `started` is when the handler began, so the logged duration includes decoding the body.
async fn store_orders(
    route: &str,
    started: Instant,
    orders: Vec<DbOrderSchema>,
    db: DbRef,
) -> HttpResponse {
    let count = orders.len();
    let response = match db.get_ref().orders().insert_orders(orders).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    };
    let elapsed = started.elapsed();
    if elapsed > SLOW_ORDER_POST {
        tracing::warn!(
            "POST {} of {} order(s) took {} ms (the orderbook worker deadline is 2000 ms)",
            route,
            count,
            elapsed.as_millis()
        );
    }
    response
}

#[tracing::instrument(
    name = "Adding new orders",
    skip(orders, db),
    fields(
    orders = ?orders
    )
)]
pub async fn post_orders(orders: Json<Vec<u8>>, db: DbRef) -> impl Responder {
    let started = Instant::now();
    let deserialized_orders = convert_gsy_node_order_schema_to_db_schema(orders.to_vec());
    store_orders("/orders", started, deserialized_orders, db).await
}

pub async fn post_normalized_orders(orders: Json<Vec<DbOrderSchema>>, db: DbRef) -> impl Responder {
    store_orders(
        "/orders-normalized",
        Instant::now(),
        orders.into_inner(),
        db,
    )
    .await
}

#[derive(Deserialize)]
pub struct OrdersParameters {
    #[serde(default)]
    market_id: Option<String>,
    #[serde(default)]
    start_time: Option<u32>,
    #[serde(default)]
    end_time: Option<u32>,
}

async fn filter_orders_from_db(
    db: DbRef,
    orders_parameters: Query<OrdersParameters>,
) -> Result<Vec<DbOrderSchema>, Error> {
    if orders_parameters.market_id.is_none()
        && orders_parameters.start_time.is_none()
        && orders_parameters.end_time.is_none()
    {
        db.get_ref().orders().get_all_orders().await
    } else {
        db.get_ref()
            .orders()
            .filter_orders(
                orders_parameters.market_id.clone(),
                orders_parameters.start_time,
                orders_parameters.end_time,
            )
            .await
    }
}

pub async fn get_orders(db: DbRef, orders_parameters: Query<OrdersParameters>) -> impl Responder {
    match filter_orders_from_db(db, orders_parameters).await {
        Ok(orders) => HttpResponse::Ok().json(orders),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
