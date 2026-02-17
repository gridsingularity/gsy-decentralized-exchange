use crate::db_api_schema;
use crate::db_api_schema::orders::{
    DbAttributes, DbBid, DbOffer, DbOrderComponent, DbOrderSchema, DbRequirements, EnergyType,
    OrderStatus,
};
use crate::utils::{h256_to_string, NODE_FLOAT_SCALING_FACTOR};
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
pub struct Requirements<AccountId32> {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Attributes<AccountId32> {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: EnergyType,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Bid<AccountId32> {
    pub buyer: AccountId32,
    pub nonce: u32,
    pub bid_component: OrderComponent,
    pub requirements: Option<Requirements<AccountId32>>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Offer<AccountId32> {
    pub seller: AccountId32,
    pub nonce: u32,
    pub offer_component: OrderComponent,
    pub attributes: Option<Attributes<AccountId32>>,
}

pub fn create_db_offer_from_node_offer(offer: Offer<AccountId32>) -> DbOffer {
	DbOffer {
		seller: offer.seller.to_string(),
		nonce: offer.nonce,
		offer_component: DbOrderComponent {
			area_uuid: h256_to_string(offer.offer_component.area_uuid),
			market_id: h256_to_string(offer.offer_component.market_id),
			time_slot: offer.offer_component.time_slot,
			creation_time: offer.offer_component.creation_time,
			energy: offer.offer_component.energy as f64 / NODE_FLOAT_SCALING_FACTOR,
			energy_rate: offer.offer_component.energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR,
		},
        attributes: offer.attributes.map(|attr| DbAttributes {
            trading_partner_id: attr.trading_partner_id.map(|id| id.to_string()),
            energy_type: attr.energy_type,
        }),
	}
}

pub fn create_db_bid_from_node_bid(bid: Bid<AccountId32>) -> DbBid {
	DbBid {
		buyer: bid.buyer.to_string(),
		nonce: bid.nonce,
		bid_component: DbOrderComponent {
			area_uuid: h256_to_string(bid.bid_component.area_uuid),
			market_id: h256_to_string(bid.bid_component.market_id),
			time_slot: bid.bid_component.time_slot,
			creation_time: bid.bid_component.creation_time,
			energy: bid.bid_component.energy as f64 / NODE_FLOAT_SCALING_FACTOR,
			energy_rate: bid.bid_component.energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR,
		},
        requirements: bid.requirements.map(|req| DbRequirements {
            trading_partner_id: req.trading_partner_id.map(|id| id.to_string()),
            energy_type: req.energy_type,
            preferred_energy_rate: req.preferred_energy_rate.map(
				|r| r as f64 / NODE_FLOAT_SCALING_FACTOR),
        }),
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
				deserialized.push(DbOrderSchema {
					_id: h256_to_string(bid_hash),
					status: order.status,
					order: db_api_schema::orders::Order::Bid(create_db_bid_from_node_bid(bid)),
				});
			},
			Order::Offer(offer) => {
				let offer_hash = H256(BlakeTwo256::hash_of(&offer).0);
				deserialized.push(DbOrderSchema {
					_id: h256_to_string(offer_hash),
					status: order.status,
					order: db_api_schema::orders::Order::Offer(create_db_offer_from_node_offer(
						offer,
					)),
				});
			},
		};
	}
	deserialized
}
