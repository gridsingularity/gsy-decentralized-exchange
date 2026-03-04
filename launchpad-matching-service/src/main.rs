use actix_web::{App, HttpServer};
use launchpad_matching_service::api::views;
use launchpad_matching_service::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to load configuration");

    HttpServer::new(|| {
        App::new()
            .service(views::health_check)
            .service(views::pay_as_bid)
            .service(views::filter_matches)
            .service(views::get_market_statistics)
            .service(views::get_markets)
    })
    .bind((
        configuration.application_host,
        configuration.application_port,
    ))?
    .run()
    .await
}
