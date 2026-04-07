use anyhow::{anyhow, Result};
use blake2_rfc::blake2b::blake2b;
use cucumber::World;
use ethers::prelude::*;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use gsy_offchain_primitives::MarketType;
use reqwest::Client;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

const DEFAULT_PRIVATE_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

#[derive(Clone, Debug)]
pub struct UserAccount {
    pub private_key: String,
    pub address: Address,
}

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct MyWorld {
    pub provider: Arc<Provider<Ws>>,
    pub chain_id: u64,
    pub http_client: Client,
    pub users: HashMap<String, UserAccount>,
    pub evm_node_url: String,
    pub orderbook_service_url: String,
    pub market_controller_address: Address,
    pub order_registry_address: Address,
    pub trade_settlement_address: Address,
    pub gsy_vault_address: Address,
    pub last_market_id: Option<[u8; 32]>,
    pub target_delivery_time: u64,
    pub buyer_id: String,
    pub seller_id: String,
    pub bid_forecast: Option<ForecastSchema>,
    pub offer_forecast: Option<ForecastSchema>,
    pub topology_schema: Option<MarketTopologySchema>,
    pub last_trade: Option<TradeSchema>,
    pub last_charlie_offer_order_id: Option<String>,
}

impl MyWorld {
    async fn new() -> Result<Self, anyhow::Error> {
        let evm_node_url =
            std::env::var("EVM_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:8545".to_string());
        let provider = Arc::new(Provider::<Ws>::connect(evm_node_url.as_str()).await?);
        let chain_id = provider.get_chainid().await?.as_u64();

        let default_private_key = std::env::var("COMMUNITY_CLIENT_PRIVATE_KEY")
            .unwrap_or_else(|_| DEFAULT_PRIVATE_KEY.to_string());

        let alice_private_key =
            std::env::var("ALICE_PRIVATE_KEY").unwrap_or_else(|_| default_private_key.clone());
        let bob_private_key =
            std::env::var("BOB_PRIVATE_KEY").unwrap_or_else(|_| default_private_key.clone());
        let charlie_private_key =
            std::env::var("CHARLIE_PRIVATE_KEY").unwrap_or_else(|_| default_private_key.clone());

        let mut users = HashMap::new();
        users.insert(
            "alice".to_string(),
            Self::build_user(alice_private_key.as_str(), chain_id)?,
        );
        users.insert(
            "bob".to_string(),
            Self::build_user(bob_private_key.as_str(), chain_id)?,
        );
        users.insert(
            "charlie".to_string(),
            Self::build_user(charlie_private_key.as_str(), chain_id)?,
        );

        let market_controller_address = Self::read_address_env("MARKET_CONTROLLER_ADDRESS")?;
        let order_registry_address = Self::read_address_env("ORDER_REGISTRY_ADDRESS")?;
        let trade_settlement_address = Self::read_address_env("TRADE_SETTLEMENT_ADDRESS")?;
        let gsy_vault_address = Self::read_address_env("GSY_VAULT_ADDRESS")?;

        Ok(Self {
            provider,
            chain_id,
            http_client: Client::new(),
            users,
            evm_node_url,
            orderbook_service_url: std::env::var("ORDERBOOK_SERVICE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            market_controller_address,
            order_registry_address,
            trade_settlement_address,
            gsy_vault_address,
            last_market_id: None,
            target_delivery_time: 0,
            buyer_id: "areaalice".to_string(),
            seller_id: "areabob".to_string(),
            bid_forecast: None,
            offer_forecast: None,
            topology_schema: None,
            last_trade: None,
            last_charlie_offer_order_id: None,
        })
    }

    fn build_user(private_key: &str, chain_id: u64) -> Result<UserAccount> {
        let wallet = private_key
            .parse::<LocalWallet>()
            .map_err(|e| anyhow!("Invalid user private key: {}", e))?
            .with_chain_id(chain_id);

        Ok(UserAccount {
            private_key: private_key.to_string(),
            address: wallet.address(),
        })
    }

    fn read_address_env(name: &str) -> Result<Address> {
        let value = std::env::var(name)
            .map_err(|_| anyhow!("Missing required environment variable {}", name))?;
        Address::from_str(value.as_str())
            .map_err(|e| anyhow!("Invalid {} address '{}': {}", name, value, e))
    }

    pub fn wallet_for_user(&self, user_name: &str) -> LocalWallet {
        self.users
            .get(user_name)
            .unwrap_or_else(|| panic!("Unknown user '{}'", user_name))
            .private_key
            .parse::<LocalWallet>()
            .expect("Failed to parse user private key")
            .with_chain_id(self.chain_id)
    }

    pub fn private_key_for_user(&self, user_name: &str) -> String {
        self.users
            .get(user_name)
            .unwrap_or_else(|| panic!("Unknown user '{}'", user_name))
            .private_key
            .clone()
    }

    pub fn generate_market_id(&self, market_type: MarketType) -> [u8; 32] {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(market_type.as_str().as_bytes());
        buffer.extend_from_slice(&self.target_delivery_time.to_be_bytes());
        blake2b(32, &[], &buffer)
            .as_bytes()
            .try_into()
            .expect("hash is 32 bytes")
    }
}
