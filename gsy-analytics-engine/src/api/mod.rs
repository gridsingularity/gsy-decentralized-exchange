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

/// Query parameters shared by the KPI endpoints. Times are unix seconds and select periods
/// by `period_start` in `[start_time, end_time)`.
#[derive(Deserialize, Debug)]
pub struct KpiQuery {
    #[serde(default)]
    pub start_time: Option<u64>,
    #[serde(default)]
    pub end_time: Option<u64>,
    #[serde(default)]
    pub community_id: Option<String>,
}

fn to_unix_seconds(value: Option<u64>, name: &str) -> Result<Option<i64>, HttpResponse> {
    value
        .map(|value| {
            i64::try_from(value)
                .map_err(|_| HttpResponse::BadRequest().body(format!("{} is out of range", name)))
        })
        .transpose()
}

impl KpiQuery {
    pub fn to_filter(&self, kpi_id: &str) -> Result<ResultsFilter, HttpResponse> {
        if let (Some(start), Some(end)) = (self.start_time, self.end_time) {
            if end <= start {
                return Err(HttpResponse::BadRequest().body("end_time must be after start_time"));
            }
        }
        Ok(ResultsFilter {
            kpi_id: kpi_id.to_string(),
            community_id: self.community_id.clone(),
            start_time: to_unix_seconds(self.start_time, "start_time")?,
            end_time: to_unix_seconds(self.end_time, "end_time")?,
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
