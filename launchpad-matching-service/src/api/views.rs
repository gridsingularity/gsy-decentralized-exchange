use crate::api::controller::{MatchController, MatchControllerBase};
use crate::api::model::{Resolution, SortOrder};
use crate::api::types::OrdersToMatch;
use crate::auth::jwt::Claims;
use actix_web::error::{InternalError, JsonPayloadError, QueryPayloadError};
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError, get, post, web};
use mongodb::Database;
use mongodb::bson::doc;
use mongodb::error::ErrorKind;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

/// Upper bound for the MongoDB ping done by `/ready`.
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

/// Time zone used by `/statistics` when the `tz` query parameter is absent.
pub const DEFAULT_TIME_ZONE: &str = "UTC";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MatchFilterQuery {
    pub user_id: String,
    pub market_id: Option<String>,
    pub start_time: u64,
    pub end_time: u64,
    pub limit: Option<i64>,
    /// Number of matches to skip (for paging).
    pub offset: Option<u64>,
    /// `asc` (default) or `desc`, on `time_slot` and then `match_id`.
    pub sort: Option<SortOrder>,
}

/// Query string of `POST /match`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MatchQuery {
    /// `true` computes and returns the matches without writing anything to
    /// MongoDB. Default `false`.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketStatisticsQuery {
    pub user_id: String,
    pub market_id: Option<String>,
    pub start_time: u64,
    pub end_time: u64,
    pub resolution: Option<Resolution>,
    /// IANA time zone name for the buckets, default `UTC`.
    pub tz: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketsQuery {
    pub user_id: String,
}

/// Response returned when a database operation fails. The error itself is
/// only logged, never sent to the client.
pub(crate) fn database_error_response() -> HttpResponse {
    HttpResponse::InternalServerError().json(json!({
        "error": "internal_error",
        "message": "A database operation failed."
    }))
}

fn invalid_timezone_response() -> HttpResponse {
    HttpResponse::BadRequest().json(json!({
        "error": "invalid_timezone",
        "message": "Unknown time zone."
    }))
}

/// Cheap shape check of a time zone name before it reaches MongoDB:
/// 1 to 64 characters out of `A-Za-z0-9_+-/`. Whether the zone exists is
/// left to MongoDB.
pub fn is_plausible_time_zone(tz: &str) -> bool {
    (1..=64).contains(&tz.len())
        && tz
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-' | b'/'))
}

/// MongoDB error code for "unrecognized time zone identifier".
pub const UNKNOWN_TIME_ZONE_ERROR_CODE: i32 = 40485;

/// True if MongoDB rejected a command because it does not know a time zone.
pub fn is_unknown_time_zone_error(error: &mongodb::error::Error) -> bool {
    matches!(
        error.kind.as_ref(),
        ErrorKind::Command(command_error) if command_error.code == UNKNOWN_TIME_ZONE_ERROR_CODE
    )
}

fn invalid_request_body(status: StatusCode, message: String) -> HttpResponse {
    let error = if status == StatusCode::PAYLOAD_TOO_LARGE {
        "payload_too_large"
    } else {
        "invalid_request"
    };
    HttpResponse::build(status).json(json!({
        "error": error,
        "message": message
    }))
}

/// `JsonConfig` accepting bodies up to `limit` bytes (the `json_body_limit`
/// setting), whose errors are JSON bodies
/// `{"error":"invalid_request","message":<serde error>}` (status 400; 413 with
/// `payload_too_large` when the body exceeds the limit).
pub fn json_config(limit: usize) -> web::JsonConfig {
    web::JsonConfig::default().limit(limit).error_handler(
        |err: JsonPayloadError, _req: &HttpRequest| {
            let status = err.status_code();
            let message = match &err {
                JsonPayloadError::Deserialize(e) => e.to_string(),
                other => other.to_string(),
            };
            let response = invalid_request_body(status, message);
            InternalError::from_response(err, response).into()
        },
    )
}

/// `QueryConfig` whose errors are JSON bodies
/// `{"error":"invalid_request","message":<serde error>}` with status 400.
pub fn query_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|err: QueryPayloadError, _req: &HttpRequest| {
        let message = match &err {
            QueryPayloadError::Deserialize(e) => e.to_string(),
            other => other.to_string(),
        };
        let response = invalid_request_body(StatusCode::BAD_REQUEST, message);
        InternalError::from_response(err, response).into()
    })
}

/// Liveness check: the process is up. Does not touch the database.
#[get("/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}

/// Readiness check: MongoDB answers a ping within `READINESS_TIMEOUT`.
#[get("/ready")]
pub async fn ready(db: web::Data<Database>) -> impl Responder {
    let ping = db.run_command(doc! { "ping": 1 }, None);
    match tokio::time::timeout(READINESS_TIMEOUT, ping).await {
        Ok(Ok(_)) => HttpResponse::Ok().finish(),
        Ok(Err(e)) => {
            eprintln!(
                "Readiness check failed, MongoDB ping returned an error: {:?}",
                e
            );
            database_unavailable_response()
        }
        Err(_) => {
            eprintln!("Readiness check failed, MongoDB ping timed out");
            database_unavailable_response()
        }
    }
}

fn database_unavailable_response() -> HttpResponse {
    HttpResponse::ServiceUnavailable().json(json!({
        "error": "database_unavailable",
        "message": "The database is not reachable."
    }))
}

#[post("/match")]
pub async fn pay_as_bid(
    _claims: Claims,
    db: web::Data<Database>,
    query: web::Query<MatchQuery>,
    orders: web::Json<OrdersToMatch>,
) -> impl Responder {
    let controller = MatchController {
        db: db.get_ref().clone(),
    };
    let persist = !query.dry_run;
    match controller
        .process_market_id_for_pay_as_bid(orders.into_inner(), persist)
        .await
    {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => {
            eprintln!("Failed to process matches in MongoDB: {:?}", e);
            database_error_response()
        }
    }
}

#[get("/matches")]
pub async fn filter_matches(
    _claims: Claims,
    db: web::Data<Database>,
    query: web::Query<MatchFilterQuery>,
) -> impl Responder {
    let controller = MatchController {
        db: db.get_ref().clone(),
    };
    let result = controller
        .filter_matches(
            query.user_id.clone(),
            query.market_id.clone(),
            query.start_time,
            query.end_time,
            query.limit,
            query.offset,
            query.sort.unwrap_or_default(),
        )
        .await;
    match result {
        Ok(matches) => HttpResponse::Ok().json(matches),
        Err(e) => {
            eprintln!("Failed to fetch matches from MongoDB: {:?}", e);
            database_error_response()
        }
    }
}

#[get("/statistics")]
pub async fn get_market_statistics(
    _claims: Claims,
    db: web::Data<Database>,
    query: web::Query<MarketStatisticsQuery>,
) -> impl Responder {
    let tz = query
        .tz
        .clone()
        .unwrap_or_else(|| DEFAULT_TIME_ZONE.to_string());
    if !is_plausible_time_zone(&tz) {
        return invalid_timezone_response();
    }
    let controller = MatchController {
        db: db.get_ref().clone(),
    };
    let result = controller
        .get_market_statistics(
            query.user_id.clone(),
            query.market_id.clone(),
            query.start_time,
            query.end_time,
            query.resolution.unwrap_or(Resolution::NoAggregation),
            tz.clone(),
        )
        .await;
    match result {
        Ok(statistics) => HttpResponse::Ok().json(statistics),
        // MongoDB rejects the pipeline with code 40485 when it does not know
        // the time zone. Every other error, including other command errors
        // and connectivity failures, stays 500.
        Err(e) if is_unknown_time_zone_error(&e) => {
            eprintln!(
                "MongoDB rejected the statistics pipeline for time zone {:?}: {:?}",
                tz, e
            );
            invalid_timezone_response()
        }
        Err(e) => {
            eprintln!("Failed to fetch market statistics from MongoDB: {:?}", e);
            database_error_response()
        }
    }
}

#[get("/markets")]
pub async fn get_markets(
    _claims: Claims,
    db: web::Data<Database>,
    query: web::Query<MarketsQuery>,
) -> impl Responder {
    let controller = MatchController {
        db: db.get_ref().clone(),
    };
    match controller.get_markets(query.user_id.clone()).await {
        Ok(market_ids) => HttpResponse::Ok().json(market_ids),
        Err(e) => {
            eprintln!("Failed to fetch unique market ids from MongoDB: {:?}", e);
            database_error_response()
        }
    }
}
