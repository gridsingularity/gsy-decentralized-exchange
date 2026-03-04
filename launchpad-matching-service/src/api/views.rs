use actix_web::{get, post, web, HttpResponse, Responder};
use crate::api::controller::{MatchController, MatchControllerBase};
use crate::api::types::OrdersToMatch;


#[get("/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

#[post("/match")]
pub async fn pay_as_bid(
    orders: web::Json<OrdersToMatch>,
) -> impl Responder {
    let controller = MatchController {};
    let result = controller.process_market_id_for_pay_as_bid(orders.into_inner()).await;
    HttpResponse::Ok().json(result)
}
