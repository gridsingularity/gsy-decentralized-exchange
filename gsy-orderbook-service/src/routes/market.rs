use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct MarketParameters {
    market_id: String,
}

#[derive(Deserialize)]
pub struct MarketFromCommunityParameters {
    community_uuid: String,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

pub async fn post_market(market: Json<MarketTopologySchema>, db: DbRef) -> impl Responder {
    match db.get_ref().markets().insert(market.to_owned()).await {
        Ok(id) => HttpResponse::Ok().json(id),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_market(db: DbRef, params: Query<MarketParameters>) -> impl Responder {
    let market_service = db.get_ref().markets();
    match market_service.filter(params.market_id.clone()).await {
        Ok(markets) => {
            get_only_one_market(markets, format!("market id ({})", params.market_id.clone()))
        }
        Err(e) => {
            println!("Error getting market: {:?}", e);
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
            params.community_uuid.clone(),
            params.start_time,
            params.end_time,
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

fn get_only_one_market(
    markets: Vec<MarketTopologySchema>,
    tracing_description: String,
) -> HttpResponse {
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
