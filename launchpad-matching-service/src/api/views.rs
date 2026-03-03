use actix_web::{post, web, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use crate::api::controller;

#[post("/match")]
pub async fn pay_as_bid(orders: web::Json<Vec<DbOrderSchema>>) -> impl Responder {
    let result = controller::process_market_id_for_pay_as_bid(orders.into_inner()).await;
    HttpResponse::Ok().json(result)
}
