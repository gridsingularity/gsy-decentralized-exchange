use gsy_offchain_storage::db::DatabaseWrapper;
use gsy_offchain_storage::startup::run;
use gsy_offchain_storage::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use std::net::TcpListener;

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_wrapper: DatabaseWrapper,
    pub api_key: String,
}

async fn spawn_app(api_key: &str) -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    // Use the in-memory storage backend so the tests run without a MongoDB instance.
    let db_wrapper = DatabaseWrapper::in_memory();
    let server =
        run(listener, db_wrapper.clone(), api_key.to_string()).expect("Failed to bind address");

    let _ = tokio::spawn(server);
    TestApp {
        address,
        db_wrapper,
        api_key: api_key.to_string(),
    }
}

pub async fn init_app() -> TestApp {
    spawn_app("").await
}

pub async fn init_app_with_api_key(api_key: &str) -> TestApp {
    spawn_app(api_key).await
}
