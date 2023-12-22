use crate::helpers::init_app;

#[tokio::test]
async fn health_check() {
    let app = init_app().await;
    let address = app.address;

    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("{}/health_check", &address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(resp.status().is_success());
    assert_eq!(Some(0), resp.content_length());
}
