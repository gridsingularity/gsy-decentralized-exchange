use crate::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::{
	InputBid, InputOffer, InputOrder, OrderComponent,
};
use crate::time_utils::get_current_timestamp_in_secs;
use anyhow::{Error, Result};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{string_to_h256, NODE_FLOAT_SCALING_FACTOR};
use subxt::{utils::AccountId32, OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::{dev, Keypair};
use tracing::info;

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

pub async fn publish_orders(
	url: String,
	forecasts: Vec<ForecastSchema>,
	market: MarketTopologySchema,
	signer: &Keypair,
) -> Result<(), Error> {
	let api = OnlineClient::<SubstrateConfig>::from_insecure_url(url).await?;

	let input_orders = create_input_orders(forecasts, market, signer);
	let register_order_tx = gsy_node::tx().orderbook_worker().insert_orders(input_orders);

	let order_submit_and_watch = api
		.tx()
		.sign_and_submit_then_watch_default(&register_order_tx, signer)
		.await?
		.wait_for_finalized_success()
		.await?;

	let transfer_event = order_submit_and_watch
		.find_first::<gsy_node::orderbook_registry::events::AllOrdersInserted>()?;

	if let Some(event) = transfer_event {
		info!("Orders publishing success: {event:?}");
	} else {
		info!("Failed to find AllOrdersInserted Event");
	}

	Ok(())
}

fn _create_bid_object(
	forecast: ForecastSchema,
	area_info: AreaTopologySchema,
	market: MarketTopologySchema,
	now: u64,
	signer: &Keypair,
) -> InputOrder<AccountId32> {
	InputOrder::Bid {
		0: InputBid {
			buyer: AccountId32::from(signer.public_key()),
			bid_component: OrderComponent {
				area_uuid: string_to_h256(area_info.area_hash.clone()),
				energy: (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
				energy_rate: (forecast.energy_kwh.abs() * BID_RATE * NODE_FLOAT_SCALING_FACTOR)
					as u64,
				market_id: string_to_h256(market.market_id.clone()),
				creation_time: now,
				time_slot: market.time_slot as u64,
			},
		},
	}
}

fn _create_offer_object(
	forecast: ForecastSchema,
	area_info: AreaTopologySchema,
	market: MarketTopologySchema,
	now: u64,
	signer: &Keypair,
) -> InputOrder<AccountId32> {
	InputOrder::Offer {
		0: InputOffer {
			seller: AccountId32::from(signer.public_key()),
			offer_component: OrderComponent {
				area_uuid: string_to_h256(area_info.area_hash.clone()),
				energy: (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
				energy_rate: (forecast.energy_kwh.abs() * OFFER_RATE * NODE_FLOAT_SCALING_FACTOR)
					as u64,
				market_id: string_to_h256(market.market_id.clone()),
				creation_time: now,
				time_slot: market.time_slot as u64,
			},
		},
	}
}

pub fn create_input_orders(
	forecasts: Vec<ForecastSchema>,
	market: MarketTopologySchema,
	signer: &Keypair,
) -> Vec<InputOrder<AccountId32>> {
	let now: u64 = get_current_timestamp_in_secs();

	let mut input_orders: Vec<InputOrder<AccountId32>> = Vec::new();

	for forecast in forecasts {
		let area_info = market.area_uuids.iter().find(|area| area.area_uuid == forecast.area_uuid);
		if area_info.is_none() {
			continue;
		}

		if forecast.energy_kwh > 0. {
			input_orders.push(_create_bid_object(
				forecast,
				area_info.unwrap().clone(),
				market.clone(),
				now,
				signer,
			));
		} else if forecast.energy_kwh < 0. {
			input_orders.push(_create_offer_object(
				forecast,
				area_info.unwrap().clone(),
				market.clone(),
				now,
				signer,
			));
		}
	}
	input_orders
}
