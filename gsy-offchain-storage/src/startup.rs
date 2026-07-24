use crate::db::DatabaseWrapper;
use crate::routes::{
    get_communities, get_forecasts, get_guarantees_of_origin, get_market,
    get_market_from_community, get_markets_in_window, get_measurements, get_orders,
    get_traded_energy, get_trades, health_check, post_forecasts, post_market, post_measurements,
    post_normalized_orders, post_normalized_trades, post_orders, post_trades,
};
use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
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
            .route("/orders-normalized", web::post().to(post_normalized_orders))
            .route("/orders", web::post().to(post_orders))
            .route("/orders", web::get().to(get_orders))
            .route("/trades-normalized", web::post().to(post_normalized_trades))
            .route("/trades", web::post().to(post_trades))
            .route("/trades", web::get().to(get_trades))
            .route("/traded-energy", web::get().to(get_traded_energy))
            .route(
                "/guarantees-of-origin-measurements",
                web::get().to(get_guarantees_of_origin),
            )
            .route("/measurements", web::post().to(post_measurements))
            .route("/measurements", web::get().to(get_measurements))
            .route("/forecasts", web::post().to(post_forecasts))
            .route("/forecasts", web::get().to(get_forecasts))
            .route("/market", web::post().to(post_market))
            .route("/market", web::get().to(get_market))
            .route("/markets", web::get().to(get_markets_in_window))
            .route(
                "/community-market",
                web::get().to(get_market_from_community),
            )
            .route("/communities", web::get().to(get_communities))
            .app_data(db_connection_wrapper.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
