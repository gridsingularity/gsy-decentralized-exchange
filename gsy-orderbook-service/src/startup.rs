use std::net::TcpListener;
use actix_web::{web, App, HttpServer};
use actix_web::dev::Server;
use crate::db::DatabaseWrapper;
use crate::routes::{health_check, get_orders, post_orders};
use tracing_actix_web::TracingLogger;


pub fn run(listener: TcpListener, db_connection_wrapper: DatabaseWrapper) -> Result<Server, std::io::Error> {
    let db_connection_wrapper = web::Data::new(db_connection_wrapper);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/orders", web::post().to(post_orders))
            .route("/orders", web::get().to(get_orders))
            .app_data(db_connection_wrapper.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}