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
		if orderbook_env_var.is_none() {
			OrderBookServiceURL{ url: "http://localhost:8080/orders".to_string() }
		}
		else {
			OrderBookServiceURL{ url: orderbook_env_var.unwrap().to_string() }
		}

	}
}
