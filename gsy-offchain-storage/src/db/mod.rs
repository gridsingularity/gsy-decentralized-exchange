use mongodb::bson::Document;

mod connection;
mod order_service;
mod trade_service;

pub mod asset_measurements_service;
mod forecasts_service;
mod market_service;
mod measurements_service;

pub use connection::*;
use mongodb::bson::doc;
pub use order_service::*;
pub use trade_service::*;

pub fn create_filter_params_with_start_end_time(
    time_slot_key: String,
    start_time: Option<u32>,
    end_time: Option<u32>,
) -> Document {
    let mut filter_params = doc! {};
    if start_time.is_some() {
        filter_params.insert(time_slot_key.clone(), doc! {"$gte": start_time.unwrap()});
    }
    if end_time.is_some() {
        if start_time.is_some() {
            filter_params.insert(
                time_slot_key.clone(),
                doc! {"$gte": start_time.unwrap(), "$lte": end_time.unwrap()},
            );
        } else {
            filter_params.insert(time_slot_key, doc! {"$lte": end_time.unwrap()});
        }
    }
    filter_params
}
