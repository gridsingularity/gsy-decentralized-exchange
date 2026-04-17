use scale_info::prelude::string::String;
use core::option_env;
use codec::alloc::string::ToString;

#[derive(Debug)]
pub struct OrderBookServiceURL {
	pub url: String,
}

impl Default for OrderBookServiceURL{
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let orderbook_env_var = option_env!("ORDERBOOK_SERVICE_URL");
		let url = normalize_orderbook_service_url(
			orderbook_env_var.unwrap_or("http://localhost:8080/orders"),
		);
		OrderBookServiceURL { url }
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
	if !url[path_start..].contains('/') {
		url.push_str("/orders");
	}
	url
}
