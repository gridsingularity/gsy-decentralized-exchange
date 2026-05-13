use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use anyhow::{Error, Result};
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use gsy_offchain_primitives::node_to_api_schema::insert_order::convert_gsy_node_order_schema_to_db_schema;
use serde::Deserialize;

#[tracing::instrument(
    name = "Adding new orders",
    skip(orders, db),
    fields(
    orders = ?orders
    )
)]
pub async fn post_orders(orders: Json<Vec<u8>>, db: DbRef) -> impl Responder {
    let deserialized_orders = convert_gsy_node_order_schema_to_db_schema(orders.to_vec());
    match db
        .get_ref()
        .orders()
        .insert_orders(deserialized_orders)
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_orders(orders: Json<Vec<DbOrderSchema>>, db: DbRef) -> impl Responder {
    match db.get_ref().orders().insert_orders(orders.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
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
