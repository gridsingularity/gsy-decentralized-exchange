use actix_web::{test, App};
use launchpad_matching_service::api::views;

#[actix_web::test]
async fn test_health_check() {
    let app = test::init_service(
        App::new()
            .service(views::health_check)
    ).await;

    let req = test::TestRequest::get().uri("/health-check").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}
