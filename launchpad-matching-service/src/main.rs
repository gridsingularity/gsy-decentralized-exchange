use launchpad_matching_service::api::views;
use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(views::pay_as_bid)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
