use scale_info::prelude::{format, string::String};
use core::option_env;

#[derive(Debug)]
pub struct OrderBookServiceURLs {
	pub orders_url: String,
	pub trades_url: String
}

impl Default for OrderBookServiceURLs{
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let orderbook_url = option_env!("ORDERBOOK_SERVICE_URL")
            .unwrap_or("http://localhost:8080");

		OrderBookServiceURLs{
			orders_url: format!("{}/orders", base_url),
			trades_url: format!("{}/trades", base_url),
		}
	}
}