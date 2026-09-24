mod steps;
mod world;

use anyhow::Result;
use cucumber::World as _;
use mongodb::options::ClientOptions;
use mongodb::Database;
use primitives::MatchingAlgorithm;
use std::env;
use std::str::FromStr;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub async fn delete_database() -> Result<()> {
    let db_url = std::env::var("MONGO_URL").unwrap_or_else(|_| {
        "mongodb://gsy:gsy@mongodb:27017/?retryWrites=true&w=majority".to_string()
    });
    let db_name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "offchain_storage".to_string());
    let options = ClientOptions::parse(&db_url).await?;
    let client = mongodb::Client::with_options(options)?;
    client.database(db_name.as_str()).drop().await?;
    info!("Deleted test database");
    Ok(())
}

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    println!("Waiting for services to start...");
    sleep(std::time::Duration::from_secs(30)).await;

    let matching_algorithm =
        env::var("MATCHING_ALGORITHM").unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
    let matching_algorithm = MatchingAlgorithm::from_str(&matching_algorithm)
        .unwrap_or_else(|error| panic!("Invalid MATCHING_ALGORITHM: {}", error));
    let feature_path = env::var("E2E_FEATURE_PATH")
        .unwrap_or_else(|_| format!("features/{}", matching_algorithm.as_str()));

    println!(
        "Running {} E2E feature(s) from {}",
        matching_algorithm, feature_path
    );

    world::MyWorld::cucumber()
        .max_concurrent_scenarios(1)
        .after(|_feature, _rule, _scenario, _ev, _world| {
            Box::pin(async move {
                delete_database().await.ok();
            })
        })
        .run_and_exit(feature_path)
        .await;
}
