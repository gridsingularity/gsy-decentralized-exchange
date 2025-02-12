use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use actix_web::{web::Json, HttpResponse, Responder, web::Query};
use serde::Deserialize;
use crate::db::DbRef;


#[derive(Deserialize)]
pub struct MarketParameters {
    market_id: String,
}

pub async fn post_market(
    market: Json<MarketTopologySchema>,
    db: DbRef,
) -> impl Responder {
    match db.get_ref().markets().insert(market.to_owned()).await {
        Ok(id) => HttpResponse::Ok().json(id),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

pub async fn get_market(db: DbRef, params: Query<MarketParameters>) -> impl Responder {
    let market_service = db.get_ref().markets();
    match market_service.filter(params.market_id.clone()).await {
        Ok(markets) => get_only_one_market(markets, params.market_id.clone()),
        Err(e) => {
            println!("Error getting market: {:?}", e);
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn get_only_one_market(markets: Vec<MarketTopologySchema>, market_id: String) -> HttpResponse {
    match markets.len() {
        0 => {
            HttpResponse::NotFound().finish()
        },
        1 => {
            let market = markets.into_iter().next().unwrap();
            HttpResponse::Ok().json(market)
        },
        _ => {
            tracing::error!("Returned multiple markets for market id {:?}", market_id);
            HttpResponse::InternalServerError().finish()
        }
    }
}