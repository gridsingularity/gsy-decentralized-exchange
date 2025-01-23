use actix_web::{web::Json, HttpResponse, Responder};
use subxt::ext::sp_core::H256;
use subxt::utils::AccountId32;

use crate::db::DbRef;
use crate::schema_insert_order::OrderSchema as OtherOrderSchema;
use codec::Decode;
use gsy_offchain_primitives::db_api_schema::orders::OrderSchema;

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
    let transcode: Vec<OtherOrderSchema<AccountId32, H256>> = Vec::<OtherOrderSchema<AccountId32, H256>>::decode(&mut &orders[..]).unwrap();
    let serialize_other_order = serde_json::to_vec(&transcode).unwrap();
    let deserialize_to_order_struct: Vec<OrderSchema> = serde_json::from_slice(&serialize_other_order).unwrap();
    match db.get_ref().orders().insert_orders(deserialize_to_order_struct).await {
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
