use actix_web::{App, HttpServer, web};
use launchpad_matching_service::api::{model, views};
use launchpad_matching_service::auth::jwt::JwtSecret;
use launchpad_matching_service::auth::views as auth_views;
use launchpad_matching_service::configuration::get_configuration;
use mongodb::Client;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to load configuration");
    let jwt_secret = web::Data::new(JwtSecret(configuration.jwt_secret.clone()));
    let json_body_limit = configuration.json_body_limit;

    // One MongoDB client for the whole process. Handlers get the database
    // handle through app_data; the client keeps its own connection pool.
    let client = Client::with_uri_str(configuration.get_connection_string())
        .await
        .expect("Failed to create MongoDB client");
    let database = web::Data::new(client.database(configuration.database_name.as_str()));

    // Availability over strictness: a missing index (for example a unique
    // index over old documents with duplicates) must not stop the service.
    if let Err(e) = model::ensure_indexes(database.get_ref()).await {
        eprintln!(
            "WARNING: could not create all MongoDB indexes, starting anyway. \
             Queries may be slow and match_id / market_data uniqueness is not \
             enforced until this is fixed: {:?}",
            e
        );
    }

    HttpServer::new(move || {
        App::new()
            .app_data(jwt_secret.clone())
            .app_data(database.clone())
            .app_data(views::json_config(json_body_limit))
            .app_data(views::query_config())
            .service(views::health_check)
            .service(views::ready)
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
