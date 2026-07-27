mod steps;
mod world;

use cucumber::World as _;
use primitives::MatchingAlgorithm;
use std::env;
use std::str::FromStr;
use tokio::time::sleep;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    println!("Waiting for services to start...");
    sleep(std::time::Duration::from_secs(30)).await;

    let matching_algorithm = env::var("MATCHING_ALGORITHM")
        .unwrap_or_else(|_| MatchingAlgorithm::default().to_string());
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
        .run_and_exit(feature_path)
        .await;
}
