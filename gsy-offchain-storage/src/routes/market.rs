use crate::db::DbRef;
use crate::routes::parse_time_range;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use primitives::db_api_schema::market::MarketSchema;
use primitives::db_api_schema::trades::{ClearingResultSchema, MarketRoleSchema};
use primitives::ewds::dto::{EwdsClearingResultDto, EwdsMarketDto};
use primitives::utils::{normalize_community_id, normalize_market_id};
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

/// Responds with `value`, or 500 if a stored market cannot be converted to its DTO
/// (UUID market ID, RFC 3339 times).
fn market_json<T: serde::Serialize>(value: anyhow::Result<T>) -> HttpResponse {
    match value {
        Ok(value) => HttpResponse::Ok().json(value),
        Err(e) => {
            tracing::error!("Failed to convert stored market: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn get_only_one_market(markets: Vec<MarketSchema>, tracing_description: String) -> HttpResponse {
    match markets.len() {
        0 => HttpResponse::NotFound().finish(),
        1 => {
            let market = markets.into_iter().next().unwrap();
            market_json(EwdsMarketDto::try_from(market))
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
    let (start_time, end_time) =
        match parse_time_range(params.start_time.as_deref(), params.end_time.as_deref()) {
            Ok(range) => range,
            Err(response) => return response,
        };
    let community_id = match params
        .community_id
        .as_deref()
        .map(normalize_community_id)
        .transpose()
    {
        Ok(community_id) => community_id,
        Err(e) => {
            tracing::error!("Invalid markets query: {:?}", e);
            return HttpResponse::BadRequest().finish();
        }
    };
    match db
        .get_ref()
        .markets()
        .filter(params.market_id.clone(), community_id, start_time, end_time)
        .await
    {
        Ok(markets) if markets.is_empty() => HttpResponse::NotFound().finish(),
        Ok(markets) => market_json(
            markets
                .into_iter()
                .map(EwdsMarketDto::try_from)
                .collect::<anyhow::Result<Vec<_>>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_market(market: Json<EwdsMarketDto>, db: DbRef) -> impl Responder {
    let market = match MarketSchema::try_from(market.into_inner()).and_then(|market| {
        normalize_community_id(&market.community_id)?;
        Ok(market)
    }) {
        Ok(market) => market,
        Err(e) => {
            tracing::error!("Invalid market: {:?}", e);
            return HttpResponse::BadRequest().finish();
        }
    };
    match db.get_ref().markets().upsert(market).await {
        Ok(saved) => market_json(EwdsMarketDto::try_from(saved)),
        Err(e) => {
            tracing::error!("Failed to upsert market: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_clearing_result(
    result: Json<EwdsClearingResultDto>,
    db: DbRef,
) -> impl Responder {
    let db_result: ClearingResultSchema = match result.into_inner().try_into() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to convert EWDS clearing result: {:?}", e);
            return HttpResponse::BadRequest().finish();
        }
    };

    match db.get_ref().clearing_results().insert(db_result).await {
        Ok(saved) => HttpResponse::Ok().json(EwdsClearingResultDto::from(saved)),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[derive(Deserialize)]
pub struct ClearingResultQuery {
    market_id: String,
}

pub async fn get_clearing_results(db: DbRef, params: Query<ClearingResultQuery>) -> impl Responder {
    let market_id = match normalize_market_id(&params.market_id) {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    match db
        .get_ref()
        .clearing_results()
        .get_by_market(&market_id)
        .await
    {
        Ok(results) => {
            let dtos: Vec<EwdsClearingResultDto> = results
                .into_iter()
                .map(EwdsClearingResultDto::from)
                .collect();
            HttpResponse::Ok().json(dtos)
        }
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
