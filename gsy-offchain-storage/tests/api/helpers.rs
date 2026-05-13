use gsy_orderbook_service::configuration::get_configuration;
use gsy_orderbook_service::db::{init_database, DatabaseWrapper};
use gsy_orderbook_service::startup::run;
use gsy_orderbook_service::telemetry::{get_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use std::net::TcpListener;
use uuid::Uuid;

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
}

pub async fn init_app() -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database_name = Uuid::new_v4().to_string();

    let db_wrapper = init_database(
        configuration.get_connection_string(),
        configuration.database_name,
    )
    .await
    .unwrap();
    let server = run(listener, db_wrapper.clone()).expect("Failed to bind address");

    let _ = tokio::spawn(server);
    TestApp {
        address,
        db_wrapper,
    }
}
