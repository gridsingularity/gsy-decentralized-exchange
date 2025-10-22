use crate::db::DbRef;
use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use gsy_offchain_primitives::node_to_api_schema::insert_trades::convert_gsy_node_trades_schema_to_db_schema;
use serde::Deserialize;

#[tracing::instrument(
    name = "Adding new trades",
    skip(trades, db),
    fields(
    trades = ?trades
    )
)]
pub async fn post_trades(trades: Json<Vec<u8>>, db: DbRef) -> impl Responder {
    let deserialized_trades = convert_gsy_node_trades_schema_to_db_schema(trades.to_vec());
    match db
        .get_ref()
        .trades()
        .insert_trades(deserialized_trades)
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_trades(trades: Json<Vec<TradeSchema>>, db: DbRef) -> impl Responder {
    match db.get_ref().trades().insert_trades(trades.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    market_id: Option<String>,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

#[tracing::instrument(name = "Retrieve trades", skip(db))]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    match db
        .get_ref()
        .trades()
        .filter_trades(
            query_params.market_id.clone(),
            query_params.start_time,
            query_params.end_time,
        )
        .await
    {
        Ok(trades) => HttpResponse::Ok().json(trades),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
