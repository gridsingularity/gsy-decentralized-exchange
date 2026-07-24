use gsy_offchain_storage::configuration::get_configuration;
use gsy_offchain_storage::db::{delete_database, init_database, DatabaseWrapper};
use gsy_offchain_storage::http_server::run_http_server;
use std::net::TcpListener;
use uuid::Uuid;

pub struct TestApp {
    pub address: String,
    pub db_wrapper: DatabaseWrapper,
    pub db_name: String,
}

pub async fn init_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database_name = Uuid::new_v4().to_string();

    let db_wrapper = init_database(
        configuration.get_connection_string(),
        configuration.database_name.clone(),
    )
    .await
    .unwrap();
    let server = run_http_server(listener, db_wrapper.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);
    TestApp {
        address,
        db_wrapper,
        db_name: configuration.database_name,
    }
}

pub async fn stop_app(app: TestApp) {
    let configuration = get_configuration().expect("Failed to read configuration");
    delete_database(configuration.get_connection_string(), app.db_name)
        .await
        .unwrap();
}
