use core::option_env;
use scale_info::prelude::{format, string::{String, ToString}};

#[derive(Debug)]
pub struct OrderBookServiceURLs {
	pub orders_url: String,
	pub trades_url: String,
	/// API key sent as the `x-api-key` header the off-chain storage now requires. Read at
	/// compile time (like the URL above) because an offchain worker cannot read host env
	/// vars at runtime; defaults to `fedecom_user` and must match the storage's key.
	pub api_key: String,
}

impl Default for OrderBookServiceURLs {
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let orderbook_url = normalize_orderbook_service_url(
			option_env!("OFFCHAIN_STORAGE_URL").unwrap_or("http://localhost:8080")
		);

		OrderBookServiceURLs {
			orders_url: format!("{}/orders", orderbook_url),
			trades_url: format!("{}/trades", orderbook_url),
			api_key: option_env!("API_KEY").unwrap_or("fedecom_user").to_string(),
		}
	}
}

fn normalize_orderbook_service_url(raw_url: &str) -> String {
	let mut url = raw_url.to_string();
	if !url.contains("://") {
		let mut normalized_url = "http://".to_string();
		normalized_url.push_str(&url);
		url = normalized_url;
	}
	let path_start = url.find("://").map(|idx| idx + 3).unwrap_or(0);
	url
}