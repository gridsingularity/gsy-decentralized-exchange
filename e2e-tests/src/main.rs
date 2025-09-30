mod steps;
mod world;

use cucumber::World as _;
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

    world::MyWorld::run("features").await;
}