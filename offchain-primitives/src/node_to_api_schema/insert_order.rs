//! Convert SCALE-encoded orders emitted by the GSY DEX Node into the
//! Order Book Storage schema specified in D3.2 §5.4.
//!
//! The on-chain representation keeps energy / price in integer units
//! (scaled by 10_000) and identifies parties via `AccountId32` and
//! markets/areas via `H256`. The converter normalises these into the
//! flat off-chain schema (string ids, floats for quantities).

use crate::db_api_schema::orders::{DbOrderSchema, OrderStatus, OrderType};
use crate::utils::h256_to_string;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{BlakeTwo256, Hash as HashT};
use subxt::utils::{AccountId32, H256};

#[derive(Serialize, Deserialize, Encode, Decode, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum Order<AccountId32> {
	Bid(Bid<AccountId32>),
	Offer(Offer<AccountId32>),
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct OrderComponent {
	pub area_uuid: H256,
	pub market_id: H256,
	pub time_slot: u64,
	pub creation_time: u64,
	pub energy: u64,
	pub energy_rate: u64,
}

#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub struct OrderSchema<AccountId32, Hash> {
	pub _id: Hash,
	pub status: OrderStatus,
	pub order: Order<AccountId32>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Bid<AccountId32> {
	pub buyer: AccountId32,
	pub nonce: u32,
	pub bid_component: OrderComponent,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Offer<AccountId32> {
	pub seller: AccountId32,
	pub nonce: u32,
	pub offer_component: OrderComponent,
}

const ENERGY_SCALE: f64 = 10_000.0;

fn bid_to_db_order(order_id: String, status: OrderStatus, bid: Bid<AccountId32>) -> DbOrderSchema {
	DbOrderSchema {
		order_id,
		order_type: OrderType::Bid,
		quantity: bid.bid_component.energy as f64 / ENERGY_SCALE,
		price_limit: bid.bid_component.energy_rate as f64 / ENERGY_SCALE,
		time_slot: bid.bid_component.time_slot.to_string(),
		market_id: h256_to_string(bid.bid_component.market_id),
		order_status: status,
		creation_time: bid.bid_component.creation_time.to_string(),
		created_by: bid.buyer.to_string(),
		energy_source_preference: None,
		energy_type: None,
		area_uuid: Some(h256_to_string(bid.bid_component.area_uuid)),
	}
}

fn offer_to_db_order(
	order_id: String,
	status: OrderStatus,
	offer: Offer<AccountId32>,
) -> DbOrderSchema {
	DbOrderSchema {
		order_id,
		order_type: OrderType::Offer,
		quantity: offer.offer_component.energy as f64 / ENERGY_SCALE,
		price_limit: offer.offer_component.energy_rate as f64 / ENERGY_SCALE,
		time_slot: offer.offer_component.time_slot.to_string(),
		market_id: h256_to_string(offer.offer_component.market_id),
		order_status: status,
		creation_time: offer.offer_component.creation_time.to_string(),
		created_by: offer.seller.to_string(),
		energy_source_preference: None,
		energy_type: None,
		area_uuid: Some(h256_to_string(offer.offer_component.area_uuid)),
	}
}

pub fn convert_gsy_node_order_schema_to_db_schema(
	serialized_orders: Vec<u8>,
) -> Vec<DbOrderSchema> {
	let transcode: Vec<OrderSchema<AccountId32, H256>> =
		Vec::<OrderSchema<AccountId32, H256>>::decode(&mut &serialized_orders[..]).unwrap();

	let mut deserialized: Vec<DbOrderSchema> = vec![];
	for order in transcode {
		match order.order {
			Order::Bid(bid) => {
				let bid_hash = H256(BlakeTwo256::hash_of(&bid).0);
				deserialized.push(bid_to_db_order(h256_to_string(bid_hash), order.status, bid));
			},
			Order::Offer(offer) => {
				let offer_hash = H256(BlakeTwo256::hash_of(&offer).0);
				deserialized.push(offer_to_db_order(
					h256_to_string(offer_hash),
					order.status,
					offer,
				));
			},
		};
	}
	deserialized
}
