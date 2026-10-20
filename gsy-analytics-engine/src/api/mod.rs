//! Read-only HTTP API over the stored KPI results. One GET endpoint per KPI.

mod health_check;
mod procurement_cost_per_kwh;

use crate::db::results::ResultsFilter;
use actix_web::dev::Server;
use actix_web::{web, App, HttpResponse, HttpServer};
use health_check::health_check;
use mongodb::bson::Document;
use mongodb::Collection;
use procurement_cost_per_kwh::get_procurement_cost_per_kwh;
use serde::Deserialize;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub type ResultsRef = web::Data<Collection<Document>>;

/// The API only serves a few small read queries; one worker per CPU would be wasteful.
const API_WORKERS: usize = 2;

/// Longest `[start_time, end_time)` range one request may ask for: 31 days, so that any
/// calendar month fits.
pub const MAX_QUERY_RANGE_SECONDS: u64 = 31 * 24 * 3600;

/// Query parameters shared by the KPI endpoints. Times are unix seconds and select periods
/// by `period_start` in `[start_time, end_time)`. Both bounds are required.
#[derive(Deserialize, Debug)]
pub struct KpiQuery {
    #[serde(default)]
    pub start_time: Option<u64>,
    #[serde(default)]
    pub end_time: Option<u64>,
    #[serde(default)]
    pub community_id: Option<String>,
}

fn to_unix_seconds(value: Option<u64>, name: &str) -> Result<i64, HttpResponse> {
    let value =
        value.ok_or_else(|| HttpResponse::BadRequest().body(format!("{} is required", name)))?;
    i64::try_from(value)
        .map_err(|_| HttpResponse::BadRequest().body(format!("{} is out of range", name)))
}

impl KpiQuery {
    pub fn to_filter(&self, kpi_id: &str) -> Result<ResultsFilter, HttpResponse> {
        let start_time = to_unix_seconds(self.start_time, "start_time")?;
        let end_time = to_unix_seconds(self.end_time, "end_time")?;
        if end_time <= start_time {
            return Err(HttpResponse::BadRequest().body("end_time must be after start_time"));
        }
        if (end_time - start_time) as u64 > MAX_QUERY_RANGE_SECONDS {
            return Err(HttpResponse::BadRequest().body(format!(
                "The time range must not exceed {} seconds (31 days)",
                MAX_QUERY_RANGE_SECONDS
            )));
        }
        Ok(ResultsFilter {
            kpi_id: kpi_id.to_string(),
            community_id: self.community_id.clone(),
            start_time: Some(start_time),
            end_time: Some(end_time),
        })
    }
}

/// Starts the API on `listener`. Signal handling is left to the caller, which stops the
/// server together with the scheduler.
pub fn run_http_server(
    listener: TcpListener,
    results: Collection<Document>,
) -> Result<Server, std::io::Error> {
    let results = web::Data::new(results);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route(
                "/kpis/procurement-cost-per-kwh",
                web::get().to(get_procurement_cost_per_kwh),
            )
            .app_data(results.clone())
    })
    .workers(API_WORKERS)
    .disable_signals()
    .listen(listener)?
    .run();
    Ok(server)
}
