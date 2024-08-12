use scale_info::prelude::string::String;
use core::env;
use codec::alloc::string::ToString;

#[derive(Debug)]
pub struct OrderBookServiceURL {
	pub url: String,
}

impl Default for OrderBookServiceURL{
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let orderbook_service_url: &'static str = env!("ORDERBOOK_SERVICE_URL", "Set the orderbook service url");
		OrderBookServiceURL{ url: orderbook_service_url.to_string() }
	}
}
