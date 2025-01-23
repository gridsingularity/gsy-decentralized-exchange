use scale_info::prelude::{format, string::String};
use core::option_env;
use codec::alloc::string::ToString;

#[derive(Debug)]
pub struct OrderBookServiceURL {
	pub url: String,
}

impl Default for OrderBookServiceURL{
	fn default() -> Self {
		// Set the environment variable "URL" for OrderBook_Service
		let base_url = option_env!("ORDERBOOK_SERVICE_URL")
            .unwrap_or("http://localhost:8080");
        
        OrderBookServiceURL { url: base_url.to_string() }
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