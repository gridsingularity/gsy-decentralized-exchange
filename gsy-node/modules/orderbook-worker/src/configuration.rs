use scale_info::prelude::string::String;
use scale_info::prelude::format;
use core::option_env;
use codec::alloc::string::ToString;

#[derive(Debug)]
pub struct OrderBookServiceURLs {
	pub orders_url: String,
	pub trades_url: String
}

impl Default for OrderBookServiceURLs{
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let orderbook_env_var = option_env!("ORDERBOOK_SERVICE_URL");
		let orderbook_url = if orderbook_env_var.is_none() { "http://localhost:8080".to_string() } else { orderbook_env_var.unwrap().to_string() };

		OrderBookServiceURLs{
			orders_url: format!("{}/orders", orderbook_url),
			trades_url: format!("{}/trades", orderbook_url),
		}
	}
}
