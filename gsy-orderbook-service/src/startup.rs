use std::net::TcpListener;
use actix_web::{web, App, HttpServer};
use actix_web::dev::Server;
use crate::db::DatabaseWrapper;
use crate::routes::{
    health_check, get_orders, post_orders, post_trades, get_trades,
    post_measurements, get_measurements, post_forecasts, get_forecasts,
    post_market, get_market
};
use tracing_actix_web::TracingLogger;


pub fn run(listener: TcpListener, db_connection_wrapper: DatabaseWrapper) -> Result<Server, std::io::Error> {
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
            .app_data(db_connection_wrapper.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}