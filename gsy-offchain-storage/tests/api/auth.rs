use crate::helpers::init_app_with_api_key;

#[tokio::test]
async fn get_orders_without_api_key_returns_401() {
    let app = init_app_with_api_key("test-key").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/orders", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(401, resp.status().as_u16());
}

#[tokio::test]
async fn get_orders_with_wrong_api_key_returns_401() {
    let app = init_app_with_api_key("test-key").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/orders", &app.address))
        .header("x-api-key", "wrong-key")
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(401, resp.status().as_u16());
}

#[tokio::test]
async fn get_orders_with_correct_api_key_is_not_unauthorized() {
    let app = init_app_with_api_key("test-key").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/orders", &app.address))
        .header("x-api-key", app.api_key.as_str())
        .send()
        .await
        .expect("Failed to execute request.");

    assert_ne!(401, resp.status().as_u16());
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn health_check_without_api_key_returns_200() {
    let app = init_app_with_api_key("fedecom_user").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, resp.status().as_u16());
}
