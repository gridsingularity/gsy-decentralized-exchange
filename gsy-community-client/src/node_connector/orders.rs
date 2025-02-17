use std::str::FromStr;
use gsy_offchain_primitives::service_to_node_schema::orders::Order;
use anyhow::{Error, Result};
use chrono::Local;
use subxt_signer::sr25519::dev;
use subxt::{
    SubstrateConfig,
    OnlineClient,
    utils::AccountId32
};
use tracing::info;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use crate::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::{
    InputOrder, OrderComponent, InputBid, InputOffer};


const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;


#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

pub async fn publish_orders(
    url: String,
    forecasts: Vec<ForecastSchema>,
    market: MarketTopologySchema
) -> Result<(), Error> {

    let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;

    let now: u64 = Local::now().timestamp() as u64;
    
    let input_orders = forecasts.into_iter().map(|forecast| {
        if (forecast.energy_kwh > 0.) {
            InputOrder::Bid {
                0: InputBid {
                    buyer: AccountId32::from_str(forecast.area_uuid.clone().as_str()).unwrap(),
                    bid_component: OrderComponent {
                        area_uuid: forecast.area_uuid.clone(),
                        energy: (forecast.energy_kwh * NODE_FLOAT_SCALING_FACTOR) as u64,
                        energy_rate: (forecast.energy_kwh * BID_RATE * NODE_FLOAT_SCALING_FACTOR) as u64,
                        market_id: market.market_id.clone(),
                        creation_time: now,
                        time_slot: market.time_slot as u64,
                    }
                }
            }
        }
        else if (forecast.energy_kwh < 0.) {
            InputOrder::Offer {
                0: InputOffer {
                    seller: AccountId32::from_str(forecast.area_uuid.clone().as_str()).unwrap(),
                    offer_component: OrderComponent {
                        area_uuid: forecast.area_uuid.clone(),
                        energy: (forecast.energy_kwh * NODE_FLOAT_SCALING_FACTOR) as u64,
                        energy_rate: (forecast.energy_kwh * OFFER_RATE * NODE_FLOAT_SCALING_FACTOR) as u64,
                        market_id: market.market_id.clone(),
                        creation_time: now,
                        time_slot: market.time_slot as u64,
                    }
                }
            }
        }
    }).collect();
let register_order_tx = gsy_node::tx().orderbook_worker().insert_orders(input_orders);

    let signer = dev::alice();
    let order_submit_and_watch = api
        .tx()
        .sign_and_submit_then_watch_default(&register_order_tx, &signer)
        .await?
        .wait_for_finalized_success()
        .await?;

    let transfer_event =
        order_submit_and_watch.find_first::<gsy_node::orderbook_registry::events::AllOrdersInserted>()?;

    if let Some(event) = transfer_event {
        info!("Orders publishing success: {event:?}");
    } else {
        info!("Failed to find AllOrdersInserted Event");
    }

    Ok(())
}
