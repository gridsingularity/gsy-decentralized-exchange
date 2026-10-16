use crate::db::DbRef;
use crate::ewds_handler::validate_trades_query_range;
use crate::routes::parse_time_range;
use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::ewds::dto::EwdsTradeDto;
use serde::Deserialize;

#[tracing::instrument(name = "Adding new trades", skip(db), fields(trades = ?trades))]
pub async fn post_trades(trades: Json<Vec<EwdsTradeDto>>, db: DbRef) -> impl Responder {
    let db_trades: Vec<DbTradeSchema> = match trades
        .into_inner()
        .into_iter()
        .map(DbTradeSchema::try_from)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to convert EWDS trade: {:?}", e);
            return HttpResponse::BadRequest().finish();
        }
    };

    match db.get_ref().trades().insert_trades(db_trades).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_trades(trades: Json<Vec<EwdsTradeDto>>, db: DbRef) -> impl Responder {
    post_trades(trades, db).await
}

#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    market_id: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
}

#[tracing::instrument(name = "Retrieve trades", skip(db))]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    let (start_time, end_time) = match parse_time_range(
        query_params.start_time.as_deref(),
        query_params.end_time.as_deref(),
    ) {
        Ok(range) => range,
        Err(response) => return response,
    };
    // Same rules as the EWDS `trades.query` operation.
    let range = match validate_trades_query_range(
        query_params.market_id.as_deref(),
        start_time,
        end_time,
    ) {
        Ok(range) => range,
        Err(error) => {
            return HttpResponse::BadRequest().body(format!("{}: {}", error.code, error.message))
        }
    };
    match db
        .get_ref()
        .trades()
        .filter_trades(
            query_params.market_id.clone(),
            range.map(|(start, _)| start),
            range.map(|(_, end)| end),
        )
        .await
    {
        Ok(trades) => {
            let dtos: Vec<EwdsTradeDto> = trades.into_iter().map(EwdsTradeDto::from).collect();
            HttpResponse::Ok().json(dtos)
        }
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
