use scale_info::prelude::{format, string::String};
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
		let base_url = option_env!("ORDERBOOK_SERVICE_URL")
            .unwrap_or("http://localhost:8080");

		OrderBookServiceURLs{
			orders_url: format!("{}/orders", orderbook_url),
			trades_url: format!("{}/trades", orderbook_url),
		}
	}
}

impl OrderBookServiceURL {
    pub fn with_endpoint(&self, endpoint: &str) -> String {
        format!(
            "{}/{}",
            self.url.trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        )
    }
}