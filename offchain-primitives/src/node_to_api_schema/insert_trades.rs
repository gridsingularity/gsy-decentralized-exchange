//! Convert SCALE-encoded trades emitted by the GSY DEX Node into the
//! Trades Storage schema specified in D3.2 §5.3. The off-chain schema
//! only persists hash references to the bid and offer (via `bid_id`
//! and `offer_id`) — the full order payloads themselves live in the
//! Order Book Storage.

use crate::db_api_schema::trades::{TradeSchema as DbTradeSchema, TradeStatus};
use crate::node_to_api_schema::insert_order::{Bid, Offer};
use crate::utils::h256_to_string;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use subxt::utils::{AccountId32, H256};

#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub struct TradeParameters<Hash> {
	pub selected_energy: u64,
	pub energy_rate: u64,
	pub trade_uuid: Hash,
}

#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub struct Trade<AccountId32, Hash> {
	pub seller: AccountId32,
	pub buyer: AccountId32,
	pub market_id: H256,
	pub trade_uuid: Hash,
	pub creation_time: u64,
	pub time_slot: u64,
	pub offer: Offer<AccountId32>,
	pub offer_hash: Hash,
	pub bid: Bid<AccountId32>,
	pub bid_hash: Hash,
	pub residual_bid: Option<Bid<AccountId32>>,
	pub residual_offer: Option<Offer<AccountId32>>,
	pub parameters: TradeParameters<Hash>,
}

const ENERGY_SCALE: f64 = 10_000.0;

pub fn convert_gsy_node_trades_schema_to_db_schema(trades: Vec<u8>) -> Vec<DbTradeSchema> {
	let transcode: Vec<Trade<AccountId32, H256>> =
		Vec::<Trade<AccountId32, H256>>::decode(&mut &trades[..]).unwrap();
	let mut deserialized: Vec<DbTradeSchema> = vec![];
	for trade in transcode {
		deserialized.push(DbTradeSchema {
			trade_id: h256_to_string(trade.trade_uuid),
			trade_quantity: trade.parameters.selected_energy as f64 / ENERGY_SCALE,
			trade_price: trade.parameters.energy_rate as f64 / ENERGY_SCALE,
			trade_timestamp: trade.creation_time.to_string(),
			time_slot: trade.time_slot.to_string(),
			market_id: h256_to_string(trade.market_id),
			trade_status: TradeStatus::Settled,
			buyer: trade.buyer.to_string(),
			seller: trade.seller.to_string(),
			bid_id: h256_to_string(trade.bid_hash),
			offer_id: h256_to_string(trade.offer_hash),
			residual_bid_id: None,
			residual_offer_id: None,
		});
	}
	deserialized
}
