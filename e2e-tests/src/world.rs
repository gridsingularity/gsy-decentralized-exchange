use anyhow::Result;
use cucumber::World;
use reqwest::Client;
use std::collections::HashMap;
use subxt::{OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct MyWorld {
    pub subxt_client: OnlineClient<SubstrateConfig>,
    pub http_client: Client,
    pub users: HashMap<String, Keypair>,
    pub last_market_id: Option<subxt::utils::H256>,
}

impl MyWorld {
    async fn new() -> Result<Self, anyhow::Error> {
        let node_url =
            std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
        let subxt_client = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;
        let http_client = Client::new();

        let mut users = HashMap::new();
        users.insert(
            "alice".to_string(),
            subxt_signer::sr25519::dev::alice(),
        );
        users.insert("bob".to_string(), subxt_signer::sr25519::dev::bob());
        users.insert("charlie".to_string(), subxt_signer::sr25519::dev::charlie());

        Ok(Self {
            subxt_client,
            http_client,
            users,
            last_market_id: None,
        })
    }
}