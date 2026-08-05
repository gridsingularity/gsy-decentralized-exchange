use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use reqwest::Client;
use std::env;

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    evm_node_url: String,
    order_registry_address: String,
    community_signer_private_key: String,
    forecast_url: String,
    measurements_url: String,
    topology_url: String,
}

impl AppState {
    fn new() -> Self {
        AppState {
            client: Client::new(),
            api_adapter: AreaMarketInfoAdapter::new(None),
            evm_node_url: env::var("EVM_NODE_URL")
                .unwrap_or_else(|_| "ws://anvil:8545".to_string()),
            order_registry_address: env::var("ORDER_REGISTRY_ADDRESS")
                .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string()),
            community_signer_private_key: env::var("COMMUNITY_CLIENT_PRIVATE_KEY").unwrap_or_else(
                |_| {
                    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80".to_string()
                },
            ),
            forecast_url: "http://localhost:8000/forecasts".to_string(),
            measurements_url: "http://localhost:8000/measurements".to_string(),
            topology_url: "http://localhost:8000/ontology".to_string(),
        }
    }

    async fn poll_and_forward(&self) {
        let _seconds_since_epoch = get_current_timestamp_in_secs();
    }
}

#[tokio::main]
async fn main() {
    let app_state = AppState::new();
    app_state.poll_and_forward().await;
}
