use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};

use crate::db::DbRef;
use codec::Decode;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use serde::Deserialize;

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
    let transcode: Vec<TradeSchema> = Vec::<TradeSchema>::decode(&mut &trades[..]).unwrap();
    let serialize_trades = serde_json::to_vec(&transcode).unwrap();
    let deserialize_to_trade_struct: Vec<TradeSchema> = serde_json::from_slice(&serialize_trades).unwrap();
    match db.get_ref().trades().insert_trades(deserialize_to_trade_struct).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish()
    }
}

#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    market_uuid: Option<String>,
    start_time: Option<u32>,
    end_time: Option<u32>
}

#[tracing::instrument(
    name = "Retrieve trades",
    skip(db),
)]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    match db.get_ref().trades().filter_trades(
        query_params.market_uuid.clone(),
        query_params.start_time,
        query_params.end_time).await {
        Ok(trades) => HttpResponse::Ok().json(trades),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        },
    }
}
