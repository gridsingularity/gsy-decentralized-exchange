use anyhow::Result;
use blake2_rfc::blake2b::blake2b;
use cucumber::World;
use gsy_offchain_primitives::MarketType;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use std::collections::HashMap;
use subxt::{utils::H256, OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::Keypair;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::utils::read_env_or;
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
	pub active_community_name: Option<String>,
	pub inter_community_market: Option<MarketTopologySchema>,
	pub inter_communities: Vec<InterCommunityParticipant>,
	// PV/demand-forecasting scenario state (single-community scenario).
	pub pv_market: Option<MarketTopologySchema>,
	pub pv_offer_forecast: Option<ForecastSchema>,
	pub demand_bid_forecast: Option<ForecastSchema>,
	/// Per-slot confidence carried by the ingested PV offer forecast; drives the
	/// confidence-lifted offer rate floor asserted after publication.
	pub pv_offer_confidence: f64,
	// PV penalty (waterfall) scenario state.
	/// The community market whose single PV asset is split across two trades.
	pub pv_penalty_market: Option<MarketTopologySchema>,
	/// The single 5.0 kWh PV production offer forecast (negative energy).
	pub pv_penalty_offer: Option<ForecastSchema>,
	/// The two demand bid forecasts (3.0 kWh and 2.0 kWh, positive energy).
	pub pv_penalty_bids: Vec<ForecastSchema>,
	/// The two settled trades on the PV area, captured from `OrderExecuted`.
	pub pv_penalty_trades: Vec<CapturedTrade>,
}

/// A settled trade captured from an `OrderExecuted` event, carrying exactly the fields the
/// penalty-waterfall assertions need: the TOP-LEVEL `trade_uuid` (the same H256 a penalty
/// references), the scaled `selected_energy`, and the `creation_time` (the waterfall sort key).
#[derive(Debug, Clone)]
pub struct CapturedTrade {
	pub trade_uuid: H256,
	pub selected_energy: u64,
	pub creation_time: u64,
}

/// Single community participating in the shared inter-community market.
#[derive(Debug, Clone)]
pub struct InterCommunityParticipant {
	pub name: String,
	/// The community's uuid, hashed into `community_id` and carried on its measurements.
	pub community_uuid: String,
	/// `community_id_from_uuid(community_uuid)` — the aggregated order's `area_uuid`.
	pub community_id: H256,
	/// The community's per-community Spot market id; the reserved inter-community id must
	/// differ from it.
	pub spot_market_id: H256,
	/// Per-asset (mixed-sign) forecasts that aggregate to `net_kwh`.
	pub forecasts: Vec<ForecastSchema>,
	/// `aggregate_net_import` over `forecasts`; >0 → Bid, <0 → Offer.
	pub net_kwh: f64,
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

/// Build the raw HTTP client used for direct calls against the off-chain storage service.
/// Every storage route but `/health_check` is behind an `x-api-key` middleware, so the key
/// (`API_KEY` env var, default `fedecom_user`) is attached as a default header, mirroring
/// `AreaMarketInfoAdapter`'s own client.
fn authorized_storage_client() -> Client {
	let api_key = read_env_or("API_KEY", "fedecom_user".to_string());
	let mut headers = HeaderMap::new();
	if let Ok(value) = HeaderValue::from_str(&api_key) {
		headers.insert("x-api-key", value);
	}
	Client::builder()
		.default_headers(headers)
		.build()
		.expect("Failed to build off-chain storage HTTP client")
}

impl MyWorld {
	async fn new() -> Result<Self, anyhow::Error> {
		let node_url =
			std::env::var("GSY_NODE_URL").unwrap_or_else(|_| "ws://127.0.0.1:9944".to_string());
		let subxt_client = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;
		let http_client = authorized_storage_client();

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
			active_community_name: None,
			inter_community_market: None, inter_communities: Vec::new(),
			pv_market: None, pv_offer_forecast: None, demand_bid_forecast: None,
			pv_offer_confidence: 0.0,
			pv_penalty_market: None, pv_penalty_offer: None,
			pv_penalty_bids: Vec::new(), pv_penalty_trades: Vec::new(),
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
