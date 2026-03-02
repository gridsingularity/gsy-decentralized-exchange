use crate::db::DatabaseWrapper;
use crate::routes::{
    get_asset_measurements, get_forecasts, get_market, get_market_from_community, get_measurements,
    get_orders, get_trades, health_check, post_asset_measurements, post_forecasts, post_market,
    post_measurements, post_orders, post_trades,
};
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(
    listener: TcpListener,
    db_connection_wrapper: DatabaseWrapper,
) -> Result<Server, std::io::Error> {
    let db_connection_wrapper = web::Data::new(db_connection_wrapper);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/orders", web::post().to(post_orders))
            .route("/orders", web::get().to(get_orders))
            .route("/trades", web::post().to(post_trades))
            .route("/trades", web::get().to(get_trades))
            .route("/measurements", web::post().to(post_measurements))
            .route("/measurements", web::get().to(get_measurements))
            .route("/forecasts", web::post().to(post_forecasts))
            .route("/forecasts", web::get().to(get_forecasts))
            .route("/market", web::post().to(post_market))
            .route("/market", web::get().to(get_market))
            .route(
                "/asset-measurements",
                web::post().to(post_asset_measurements),
            )
            .route("/asset-measurements", web::get().to(get_asset_measurements))
            .route(
                "/community-market",
                web::get().to(get_market_from_community),
            )
            .app_data(db_connection_wrapper.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
