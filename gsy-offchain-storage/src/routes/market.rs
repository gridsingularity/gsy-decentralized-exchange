use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::market::MarketSchema;
use gsy_offchain_primitives::db_api_schema::trades::{ClearingResultSchema, MarketRoleSchema};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MarketsQuery {
    market_id: Option<String>,
    community_id: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[derive(Deserialize)]
pub struct MarketQuery {
    market_id: String,
}

pub async fn get_market(db: DbRef, params: Query<MarketQuery>) -> impl Responder {
    match db
        .get_ref()
        .markets()
        .filter(Some(params.market_id.clone()), None, None, None)
        .await
    {
        Ok(markets) => get_only_one_market(
            markets,
            format!("market id ({})", params.market_id.as_str()),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn get_only_one_market(markets: Vec<MarketSchema>, tracing_description: String) -> HttpResponse {
    match markets.len() {
        0 => HttpResponse::NotFound().finish(),
        1 => {
            let market = markets.into_iter().next().unwrap();
            HttpResponse::Ok().json(market)
        }
        _ => {
            tracing::error!(
                "Returned multiple markets for market id {:?}",
                tracing_description
            );
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn get_markets(db: DbRef, params: Query<MarketsQuery>) -> impl Responder {
    match db
        .get_ref()
        .markets()
        .filter(
            params.market_id.clone(),
            params.community_id.clone(),
            params.start_time.clone(),
            params.end_time.clone(),
        )
        .await
    {
        Ok(markets) if markets.is_empty() => HttpResponse::NotFound().finish(),
        Ok(markets) => HttpResponse::Ok().json(markets),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_clearing_result(result: Json<ClearingResultSchema>, db: DbRef) -> impl Responder {
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

pub async fn get_clearing_results(db: DbRef, params: Query<ClearingResultQuery>) -> impl Responder {
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
