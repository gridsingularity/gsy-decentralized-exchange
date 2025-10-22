use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use crate::db::DbRef;
use gsy_offchain_primitives::node_to_api_schema::insert_trades::convert_gsy_node_trades_schema_to_db_schema;
use serde::Deserialize;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;


#[tracing::instrument(
    name = "Adding new trades",
    skip(trades, db),
    fields(
    trades = ?trades
    )
)]
pub async fn post_trades(
    trades: Json<Vec<u8>>,
    db: DbRef,
) -> impl Responder {
    let deserialized_trades = convert_gsy_node_trades_schema_to_db_schema(trades.to_vec());
    for trade in deserialized_trades.clone() {
        let _ = db.get_ref().orders().update_order_by_area_market_id(
            trade.market_id.clone(), trade.offer.offer_component.area_uuid.clone());
        let _ = db.get_ref().orders().update_order_by_area_market_id(
            trade.market_id.clone(), trade.bid.bid_component.area_uuid.clone());
    }
    match db.get_ref().trades().insert_trades(deserialized_trades).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}


pub async fn post_normalized_trades(
    trades: Json<Vec<TradeSchema>>,
    db: DbRef,
) -> impl Responder {
    match db.get_ref().trades().insert_trades(trades.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}


#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    market_id: Option<String>,
    start_time: Option<u32>,
    end_time: Option<u32>
}

#[tracing::instrument(
    name = "Retrieve trades",
    skip(db),
)]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    match db.get_ref().trades().filter_trades(
        query_params.market_id.clone(),
        query_params.start_time,
        query_params.end_time).await {
        Ok(trades) => HttpResponse::Ok().json(trades),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}