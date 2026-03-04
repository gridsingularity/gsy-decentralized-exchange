use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Serialize, Deserialize};
use crate::api::controller::{MatchController, MatchControllerBase};
use crate::api::types::OrdersToMatch;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MatchFilterQuery {
    pub user_id: String,
    pub market_id: Option<String>,
    pub start_time: u64,
    pub end_time: u64,
    pub limit: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketStatisticsQuery {
    pub user_id: String,
    pub market_id: Option<String>,
    pub start_time: u64,
    pub end_time: u64,
}

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

#[get("/matches")]
pub async fn filter_matches(
    query: web::Query<MatchFilterQuery>,
) -> impl Responder {
    let controller = MatchController {};
    let result = controller.filter_matches(
        query.user_id.clone(),
        query.market_id.clone(),
        query.start_time,
        query.end_time,
        query.limit,
    ).await;
    HttpResponse::Ok().json(result)
}

#[get("/statistics")]
pub async fn get_market_statistics(
    query: web::Query<MarketStatisticsQuery>,
) -> impl Responder {
    let controller = MatchController {};
    let result = controller.get_market_statistics(
        query.user_id.clone(),
        query.market_id.clone(),
        query.start_time,
        query.end_time,
    ).await;
    HttpResponse::Ok().json(result)
}
