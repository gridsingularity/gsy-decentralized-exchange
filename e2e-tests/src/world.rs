use anyhow::Result;
use blake2_rfc::blake2b::blake2b;
use cucumber::World;
use reqwest::Client;
use std::collections::HashMap;
use subxt::{utils::H256, OnlineClient, SubstrateConfig};
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
	pub target_delivery_time: u64,
}

impl MyWorld {
	async fn new() -> Result<Self, anyhow::Error> {
		let node_url =
			std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
		let subxt_client = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;
		let http_client = Client::new();

		let mut users = HashMap::new();
		users.insert("alice".to_string(), subxt_signer::sr25519::dev::alice());
		users.insert("bob".to_string(), subxt_signer::sr25519::dev::bob());
		users.insert("charlie".to_string(), subxt_signer::sr25519::dev::charlie());

		Ok(Self {
			subxt_client,
			http_client,
			users,
			last_market_id: None,
			target_delivery_time: {
				let now = std::time::SystemTime::now()
					.duration_since(std::time::UNIX_EPOCH)
					.unwrap()
					.as_secs();
				let hour_as_secs = 3600;
				let spot_open_offset_secs = 120 * 60;
				let target_hour_start =
					((now + spot_open_offset_secs) / hour_as_secs) * hour_as_secs;
				target_hour_start
			},
		})
	}

	pub fn generate_market_id(&self, market_type: &str) -> H256 {
		let mut buffer = Vec::new();
		buffer.extend_from_slice(market_type.as_bytes());
		buffer.extend_from_slice(&self.target_delivery_time.to_be_bytes());
		let hash_bytes: [u8; 32] =
			blake2b(32, &[], &buffer).as_bytes().try_into().expect("hash is 32 bytes");
		H256(hash_bytes)
	}
}
