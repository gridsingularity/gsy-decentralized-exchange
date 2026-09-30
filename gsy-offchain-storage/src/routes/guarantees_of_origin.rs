use crate::certificates::query::{guarantees_of_origin, validate_window};
use crate::db::DbRef;
use actix_web::{web::Query, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GuaranteesOfOriginParams {
    /// Lower bound (inclusive) on when the trade reached `Executed`, in unix seconds.
    /// Required: an absent bound would scan every trade ever validated.
    start_time: Option<u64>,
    /// Upper bound (exclusive) on when the trade reached `Executed`. Defaults to
    /// `start_time + 900`; must be after `start_time`, at most 900 seconds later.
    end_time: Option<u64>,
}

/// Derive `local_origin_record` certificates from trades validated as `Executed`
/// within the window. See [`guarantees_of_origin`] for the semantics.
#[tracing::instrument(name = "Retrieve guarantees of origin", skip(db))]
pub async fn get_guarantees_of_origin(
    db: DbRef,
    query_params: Query<GuaranteesOfOriginParams>,
) -> impl Responder {
    let window = match validate_window(query_params.start_time, query_params.end_time) {
        Ok(window) => window,
        Err(reason) => return HttpResponse::BadRequest().body(reason),
    };
    match guarantees_of_origin(db.get_ref(), window).await {
        Ok(records) => HttpResponse::Ok().json(records),
        Err(e) => {
            tracing::error!("Failed to derive guarantees of origin: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
