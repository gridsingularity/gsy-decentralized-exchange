use actix_web::{web::Json, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use crate::db::DbRef;
use gsy_offchain_primitives::node_to_api_schema::insert_order::convert_gsy_node_order_schema_to_db_schema;


#[tracing::instrument(
    name = "Adding new orders",
    skip(orders, db),
    fields(
    orders = ?orders
    )
)]
pub async fn post_orders(
    orders: Json<Vec<u8>>,
    db: DbRef,
) -> impl Responder {
    let deserialized_orders = convert_gsy_node_order_schema_to_db_schema(orders.to_vec());
    match db.get_ref().orders().insert_orders(deserialized_orders).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}


pub async fn post_normalized_orders(
    orders: Json<Vec<DbOrderSchema>>,
    db: DbRef,
) -> impl Responder {
    match db.get_ref().orders().insert_orders(orders.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

#[tracing::instrument(
    name = "Fetching all orders",
    skip(db),
)]
pub async fn get_orders(db: DbRef) -> impl Responder {
    match db.get_ref().orders().get_all_orders().await {
        Ok(orders) => HttpResponse::Ok().json(orders),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}
