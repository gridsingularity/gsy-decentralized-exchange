use actix_web::{get, post, web, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use crate::api::controller::{MatchController, MatchControllerBase};

#[get("/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

#[post("/match")]
pub async fn pay_as_bid(
    orders: web::Json<Vec<DbOrderSchema>>,
) -> impl Responder {
    let controller = MatchController {};
    let result = controller.process_market_id_for_pay_as_bid(orders.into_inner()).await;
    HttpResponse::Ok().json(result)
}
