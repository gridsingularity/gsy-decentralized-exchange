use actix_web::{App, HttpServer, web};
use launchpad_matching_service::api::views;
use launchpad_matching_service::auth::jwt::JwtSecret;
use launchpad_matching_service::auth::views as auth_views;
use launchpad_matching_service::configuration::get_configuration;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to load configuration");
    let jwt_secret = web::Data::new(JwtSecret(configuration.jwt_secret.clone()));

    HttpServer::new(move || {
        App::new()
            .app_data(jwt_secret.clone())
            .service(views::health_check)
            .service(auth_views::get_token)
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
