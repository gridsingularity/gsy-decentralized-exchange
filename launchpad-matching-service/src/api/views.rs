use crate::api::controller::{MatchController, MatchControllerBase};
use crate::api::model::Resolution;
use crate::api::types::OrdersToMatch;
use crate::auth::jwt::Claims;
use actix_web::{HttpResponse, Responder, get, post, web};
use serde::{Deserialize, Serialize};

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
    pub resolution: Option<Resolution>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketsQuery {
    pub user_id: String,
}

#[get("/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

#[post("/match")]
pub async fn pay_as_bid(_claims: Claims, orders: web::Json<OrdersToMatch>) -> impl Responder {
    let controller = MatchController {};
    let result = controller
        .process_market_id_for_pay_as_bid(orders.into_inner())
        .await;
    HttpResponse::Ok().json(result)
}

#[get("/matches")]
pub async fn filter_matches(_claims: Claims, query: web::Query<MatchFilterQuery>) -> impl Responder {
    let controller = MatchController {};
    let result = controller
        .filter_matches(
            query.user_id.clone(),
            query.market_id.clone(),
            query.start_time,
            query.end_time,
            query.limit,
        )
        .await;
    HttpResponse::Ok().json(result)
}

#[get("/statistics")]
pub async fn get_market_statistics(_claims: Claims, query: web::Query<MarketStatisticsQuery>) -> impl Responder {
    let controller = MatchController {};
    let result = controller
        .get_market_statistics(
            query.user_id.clone(),
            query.market_id.clone(),
            query.start_time,
            query.end_time,
            query.resolution.unwrap_or(Resolution::NoAggregation),
        )
        .await;
    HttpResponse::Ok().json(result)
}

#[get("/markets")]
pub async fn get_markets(_claims: Claims, query: web::Query<MarketsQuery>) -> impl Responder {
    let controller = MatchController {};
    let result = controller.get_markets(query.user_id.clone()).await;
    HttpResponse::Ok().json(result)
}
