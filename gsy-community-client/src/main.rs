use gsy_community_client::order_events::start_order_event_subscriber;
use primitives::ewds::env_var;
use primitives::log::setup_logging;
use tracing::{error, info};

fn ewds_handler_enabled() -> bool {
    env_var("EWDS_ENABLE_HANDLER")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() {
    setup_logging("gsy-community-client", "info");

    if !ewds_handler_enabled() {
        info!("EWDS order event subscriber disabled");
        return;
    }
    if let Err(error) = start_order_event_subscriber().await {
        error!("EWDS order event subscriber stopped: {:#}", error);
        std::process::exit(1);
    }
}
