use crate::db::DatabaseWrapper;
use crate::routes::{
    get_assets, get_clearing_results, get_communities, get_facilities, get_flexibility_orders,
    get_forecasts, get_market, get_market_roles, get_markets, get_measurement_points,
    get_measurements, get_orders, get_pilot_sites, get_sites, get_tariffs, get_timeseries,
    get_trades, health_check, post_assets, post_clearing_result, post_community, post_facility,
    post_flexibility_orders, post_forecasts, post_market, post_market_role,
    post_measurement_points, post_measurements, post_normalized_orders,
    post_normalized_trades, post_orders, post_pilot_site, post_site, post_tariff, post_timeseries,
    post_trades,
};
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use anyhow::{Error, Result};
use std::net::TcpListener;
use tracing::info;
use tracing_actix_web::TracingLogger;

/// Start http server that listens to exposed API endpoints
pub fn run_http_server(
    listener: TcpListener,
    db_connection_wrapper: DatabaseWrapper,
) -> Result<Server, std::io::Error> {
    let db_connection_wrapper = web::Data::new(db_connection_wrapper);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            // Order Book Storage (D3.2 section 5.4)
            .route("/orders-normalized", web::post().to(post_normalized_orders))
            .route("/orders", web::post().to(post_orders))
            .route("/orders", web::get().to(get_orders))
            .route(
                "/flexibility-orders",
                web::post().to(post_flexibility_orders),
            )
            .route("/flexibility-orders", web::get().to(get_flexibility_orders))
            .route("/tariffs", web::post().to(post_tariff))
            .route("/tariffs", web::get().to(get_tariffs))
            // Trades Storage (D3.2 section 5.3)
            .route("/trades-normalized", web::post().to(post_normalized_trades))
            .route("/trades", web::post().to(post_trades))
            .route("/trades", web::get().to(get_trades))
            .route("/market", web::get().to(get_market))
            .route("/markets", web::post().to(post_market))
            .route("/markets", web::get().to(get_markets))
            .route("/clearing-results", web::post().to(post_clearing_result))
            .route("/clearing-results", web::get().to(get_clearing_results))
            .route("/market-roles", web::post().to(post_market_role))
            .route("/market-roles", web::get().to(get_market_roles))
            // Measurements Storage (D3.2 section 5.2)
            .route("/measurements", web::post().to(post_measurements))
            .route("/measurements", web::get().to(get_measurements))
            .route("/forecasts", web::post().to(post_forecasts))
            .route("/forecasts", web::get().to(get_forecasts))
            .route(
                "/measurement-points",
                web::post().to(post_measurement_points),
            )
            .route("/measurement-points", web::get().to(get_measurement_points))
            .route("/timeseries", web::post().to(post_timeseries))
            .route("/timeseries", web::get().to(get_timeseries))
            // Grid Topology and Market Storage (D3.2 section 5.1)
            .route("/assets", web::post().to(post_assets))
            .route("/assets", web::get().to(get_assets))
            .route("/pilot-sites", web::post().to(post_pilot_site))
            .route("/pilot-sites", web::get().to(get_pilot_sites))
            .route("/communities", web::post().to(post_community))
            .route("/communities", web::get().to(get_communities))
            .route("/sites", web::post().to(post_site))
            .route("/sites", web::get().to(get_sites))
            .route("/facilities", web::post().to(post_facility))
            .route("/facilities", web::get().to(get_facilities))
            .app_data(db_connection_wrapper.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}

/// Create a TCP listener and run http server that exposes API endpoints
pub async fn start_server(
    address: &str,
    db_connection_wrapper: DatabaseWrapper,
) -> Result<(), Error> {
    let listener = TcpListener::bind(address).expect("Failed to bind");

    info!("Server listening on {}", listener.local_addr().unwrap());

    match run_http_server(listener, db_connection_wrapper)?.await {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}
