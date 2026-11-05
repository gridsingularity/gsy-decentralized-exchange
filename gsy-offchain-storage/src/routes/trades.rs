use crate::db::DbRef;
use crate::ewds_handler::validate_trades_query_range;
use crate::routes::parse_time_range;
use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use anyhow::{Context, Result};
use mongodb::bson::doc;
use primitives::db_api_schema::trades::DbTradeSchema;
use primitives::ewds::dto::EwdsTradeDto;
use primitives::utils::{
    bytes16_to_hex, normalize_market_id, parse_nonzero_uuid_or_hex_bytes16,
    parse_uuid_or_hex_bytes16,
};
use serde::Deserialize;
use std::collections::HashSet;

fn normalize_imported_trade(mut trade: DbTradeSchema) -> Result<DbTradeSchema> {
    for (name, id) in [
        ("tradeId", &mut trade.trade_uuid),
        ("marketId", &mut trade.market_id),
        ("bidId", &mut trade.bid_hash),
        ("offerId", &mut trade.offer_hash),
        ("buyerId", &mut trade.buyer),
        ("sellerId", &mut trade.seller),
    ] {
        *id = bytes16_to_hex(parse_nonzero_uuid_or_hex_bytes16(id).with_context(|| {
            format!("Invalid {name}: expected a nonzero UUID or bytes16 hex ID")
        })?);
    }
    for (name, residual) in [
        ("residualBidId", &mut trade.residual_bid_id),
        ("residualOfferId", &mut trade.residual_offer_id),
    ] {
        *residual = residual
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(|id| parse_uuid_or_hex_bytes16(id).with_context(|| format!("Invalid {name}")))
            .transpose()?
            .filter(|id| *id != [0; 16])
            .map(bytes16_to_hex);
    }
    Ok(trade)
}

#[tracing::instrument(name = "Adding new trades", skip(db), fields(trades = ?trades))]
pub async fn post_trades(trades: Json<Vec<EwdsTradeDto>>, db: DbRef) -> impl Responder {
    let db_trades: Vec<DbTradeSchema> = match trades
        .into_inner()
        .into_iter()
        .map(|trade| DbTradeSchema::try_from(trade).and_then(normalize_imported_trade))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to convert EWDS trade: {:?}", e);
            return HttpResponse::BadRequest().body(e.to_string());
        }
    };

    let mut ids = HashSet::new();
    if db_trades
        .iter()
        .any(|trade| !ids.insert(trade.trade_uuid.clone()))
    {
        return HttpResponse::Conflict().body("Duplicate tradeId in import batch");
    }
    match db
        .trades()
        .count_documents(doc! {"trade_uuid": {"$in": ids.into_iter().collect::<Vec<_>>()}})
        .await
    {
        Ok(0) => {}
        Ok(_) => return HttpResponse::Conflict().body("Trade already exists"),
        Err(error) => {
            tracing::error!("Failed to check trade import conflicts: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }
    match db.get_ref().trades().insert_trades(db_trades).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(error) if format!("{error:#}").contains("E11000") => {
            HttpResponse::Conflict().body("Trade already exists")
        }
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
    let market_id = match query_params
        .market_id
        .as_deref()
        .map(normalize_market_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    // Same rules as the EWDS `trades.query` operation.
    let range = match validate_trades_query_range(market_id.as_deref(), start_time, end_time) {
        Ok(range) => range,
        Err(error) => {
            return HttpResponse::BadRequest().body(format!("{}: {}", error.code, error.message))
        }
    };
    match db
        .get_ref()
        .trades()
        .filter_trades(
            market_id,
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
