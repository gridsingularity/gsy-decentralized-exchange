use anyhow::Result;
use blake2_rfc::blake2b::blake2b;
use cucumber::World;
use gsy_offchain_primitives::MarketType;
use reqwest::Client;
use std::collections::HashMap;
use subxt::{utils::H256, OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;

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
	pub buyer_id: String,
	pub seller_id: String,
	pub buyer_hash: Option<String>,
	pub seller_hash: Option<String>,
	pub bid_forecast: Option<ForecastSchema>,
	pub offer_forecast: Option<ForecastSchema>,
	pub topology_schema: Option<MarketTopologySchema>,
	pub community_client_api: AreaMarketInfoAdapter,
	pub community_uuid: Option<String>,
	pub community_markets: Vec<CommunityMarket>,
	pub cross_communities: Vec<CrossCommunity>,
	pub initial_trade_energy: Option<u64>,
	pub residual_trade_energy: Option<u64>,
}

// Community state used by the cross-community matching scenario.
#[derive(Debug, Clone)]
pub struct CrossCommunity {
	pub name: String,
	pub market_id: H256,
	pub topology: MarketTopologySchema,
	/// Positive-energy forecasts, submitted as bids signed by "charlie".
	pub bid_forecasts: Vec<ForecastSchema>,
	/// Negative-energy forecasts, submitted as offers signed by "bob".
	pub offer_forecasts: Vec<ForecastSchema>,
}

/// Community market state used by the parallel-markets scenario.
#[derive(Debug, Clone)]
pub struct CommunityMarket {
	pub name: String,
	pub market_id: H256,
	pub topology: MarketTopologySchema,
	pub buyer_area: String,
	pub seller_area: String,
	pub buyer_hash: String,
	pub seller_hash: String,
	pub bid_forecast: ForecastSchema,
	pub offer_forecast: ForecastSchema,
}

impl MyWorld {
	async fn new() -> Result<Self, anyhow::Error> {
		let node_url =
			std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
		let subxt_client = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;
		let http_client = Client::new();

		let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
			.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

		// Setting up dedicated trading accounts. "alice" is not used as a trading user, but only
		// as the sudo/root and matching-engine operator account. "bob" (seller) and "charlie"
		// (buyer) are pre-funded dev accounts in genesis, so they can cover transaction fees and
		// collateral.
		let mut users = HashMap::new();
		users.insert("bob".to_string(), subxt_signer::sr25519::dev::bob());
		users.insert("charlie".to_string(), subxt_signer::sr25519::dev::charlie());

		Ok(Self {
			subxt_client, http_client, users, last_market_id: None, target_delivery_time: 0,
			buyer_id: "areaAlice".to_string(), seller_id: "areaBob".to_string(),
			buyer_hash: None, seller_hash: None,
			bid_forecast: None, offer_forecast: None, topology_schema: None,
			community_client_api: AreaMarketInfoAdapter::new(Some(orderbook_url)), community_uuid: None,
			community_markets: Vec::new(), cross_communities: Vec::new(),
			initial_trade_energy: None, residual_trade_energy: None,
		})
	}

	pub fn generate_market_id(&self, community_name: &str, market_type: MarketType) -> H256 {
		let mut buffer = Vec::new();
		buffer.extend_from_slice(community_name.as_bytes());
		buffer.extend_from_slice(market_type.as_str().as_bytes());
		buffer.extend_from_slice(&self.target_delivery_time.to_be_bytes());
		let hash_bytes: [u8; 32] =
			blake2b(32, &[], &buffer).as_bytes().try_into().expect("hash is 32 bytes");
		H256(hash_bytes)
	}
}
