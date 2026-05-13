use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::trades::{ClearingResultSchema, MarketRoleSchema};
use gsy_offchain_primitives::db_api_schema::market::MarketSchema;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MarketParameters {
    market_id: String,
}

#[derive(Deserialize)]
pub struct MarketFromCommunityParameters {
    community_id: String,
    start_time: Option<String>,
    end_time: Option<String>,
}

pub async fn post_market(market: Json<MarketSchema>, db: DbRef) -> impl Responder {
    match db.get_ref().markets().insert(market.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_market(db: DbRef, params: Query<MarketParameters>) -> impl Responder {
    let market_service = db.get_ref().markets();
    match market_service.filter(params.market_id.clone()).await {
        Ok(markets) if markets.is_empty() => HttpResponse::NotFound().finish(),
        Ok(markets) => HttpResponse::Ok().json(markets),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn get_market_from_community(
    db: DbRef,
    params: Query<MarketFromCommunityParameters>,
) -> impl Responder {
    let market_service = db.get_ref().markets();
    match market_service
        .get_community_market(
            params.community_id.clone(),
            params.start_time.clone(),
            params.end_time.clone(),
        )
        .await
    {
        Ok(markets) => HttpResponse::Ok().json(markets),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_clearing_result(
    result: Json<ClearingResultSchema>,
    db: DbRef,
) -> impl Responder {
    match db
        .get_ref()
        .clearing_results()
        .insert(result.to_owned())
        .await
    {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[derive(Deserialize)]
pub struct ClearingResultQuery {
    market_id: String,
}

pub async fn get_clearing_results(
    db: DbRef,
    params: Query<ClearingResultQuery>,
) -> impl Responder {
    match db
        .get_ref()
        .clearing_results()
        .get_by_market(&params.market_id)
        .await
    {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_market_role(role: Json<MarketRoleSchema>, db: DbRef) -> impl Responder {
    match db.get_ref().market_roles().insert(role.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_market_roles(db: DbRef) -> impl Responder {
    match db.get_ref().market_roles().get_all().await {
        Ok(roles) => HttpResponse::Ok().json(roles),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
