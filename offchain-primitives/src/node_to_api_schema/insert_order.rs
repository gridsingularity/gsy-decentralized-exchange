use crate::db_api_schema::orders::{DbAttributes, DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus};
use crate::utils::{h256_to_string, NODE_FLOAT_SCALING_FACTOR};
use serde::{Deserialize, Serialize};
use subxt::utils::{AccountId32, H256};


#[derive(Serialize, Deserialize, Clone)]
struct NodeOrderSchema<AccountId32, Hash> {
	pub order_id: Hash,
	pub order_type: OrderEnum,
	pub status: OrderStatus,
	pub area_uuid: H256,
	pub market_id: H256,
	pub time_slot: u64,
	pub creation_time: u64,
	pub energy: u64,
	pub energy_rate: u64,
	pub created_by: AccountId32,
	pub requirements: Option<NodeRequirements<AccountId32>>,
	pub attributes: Option<NodeAttributes<AccountId32>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NodeRequirements<AccountId32> {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NodeAttributes<AccountId32> {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: EnergyType,
}

// TODO: Adapt the commented method as soon as migration to the Energy Web chain is complete.
// pub fn convert_gsy_node_order_schema_to_db_schema(
// 	serialized_orders: Vec<u8>,
// ) -> Vec<DbOrderSchema> {
// 	let transcode: Vec<NodeOrderSchema<AccountId32, H256>> =
// 		Vec::<NodeOrderSchema<AccountId32, H256>>::decode(&mut &serialized_orders[..]).unwrap();
//
// 	transcode.into_iter().map(|order| {
// 		DbOrderSchema {
// 			order_id: h256_to_string(order.order_id),
// 			order_type: order.order_type,
// 			created_by: order.created_by.to_string(),
// 			market_id: h256_to_string(order.market_id),
// 			area_uuid: h256_to_string(order.area_uuid),
// 			time_slot: order.time_slot,
// 			creation_time: order.creation_time,
// 			status: order.status,
// 			energy_rate: order.energy_rate as f64 / NODE_FLOAT_SCALING_FACTOR,
// 			energy_kWh: order.energy as f64 / NODE_FLOAT_SCALING_FACTOR,
// 			requirements: order.requirements.map(|req| DbRequirements {
// 				trading_partner_id: req.trading_partner_id.map(|id: AccountId32| id.to_string()),
// 				energy_type: req.energy_type,
// 				preferred_energy_rate: req.preferred_energy_rate.map(|r| r as f64 / NODE_FLOAT_SCALING_FACTOR),
// 			}),
// 			attributes: order.attributes.map(|attr| DbAttributes {
// 				trading_partner_id: attr.trading_partner_id.map(|id: AccountId32| id.to_string()),
// 				energy_type: attr.energy_type,
// 			})
// 		}
// 	}).collect()
//
// }
