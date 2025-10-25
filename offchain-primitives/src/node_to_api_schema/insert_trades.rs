use serde::{Deserialize, Serialize};
use codec::{Decode, Encode};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::CheckedConversion;
use subxt::utils::AccountId32;
use crate::db_api_schema;
use crate::node_to_api_schema::insert_order::{
    Offer, Bid, create_db_offer_from_node_offer, create_db_bid_from_node_bid};
use crate::db_api_schema::trades::{
    TradeSchema as DbTradeSchema, TradeStatus, TradeParameters as DbTradeParameters};
use crate::utils::h256_to_string;
use uuid::Uuid;


#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub struct TradeParameters<Hash> {
    /// The amount of energy that is traded.
    pub selected_energy: u64,
    /// The price of the traded energy.
    pub energy_rate: u64,
    /// The trade hash.
    pub trade_uuid: Hash,
}

#[derive(Serialize, Deserialize, Encode, Decode, Clone)]
pub struct Trade<AccountId32, Hash> {
    pub seller: AccountId32,
    pub buyer: AccountId32,
    pub market_id: u8,
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


pub fn convert_gsy_node_trades_schema_to_db_schema(trades: Vec<u8>) -> Vec<DbTradeSchema> {
    let transcode: Vec<Trade<AccountId32, H256>> = Vec::<Trade<AccountId32, H256>>::decode(
        &mut &trades[..]).unwrap();
    let mut deserialized: Vec<db_api_schema::trades::TradeSchema> = vec!();
    for trade in transcode {
        deserialized.push(db_api_schema::trades::TradeSchema {
            _id: Uuid::new_v4().to_string(),
            status: TradeStatus::Settled,
            seller: trade.seller.to_string(),
            buyer: trade.buyer.to_string(),
            market_id: trade.market_id.to_string(),
            time_slot: trade.time_slot,
            trade_uuid: trade.trade_uuid.to_string(),
            creation_time: trade.creation_time,
            offer: create_db_offer_from_node_offer(trade.offer),
            offer_hash: h256_to_string(trade.offer_hash),
            bid: create_db_bid_from_node_bid(trade.bid),
            bid_hash: h256_to_string(trade.bid_hash),
            residual_offer: match trade.residual_offer {
                Some(residual_offer) => create_db_offer_from_node_offer(residual_offer).checked_into(),
                None => None
            },
            residual_bid: match trade.residual_bid {
                Some(residual_bid) => create_db_bid_from_node_bid(residual_bid).checked_into(),
                None => None
            },
            parameters: DbTradeParameters {
                selected_energy: trade.parameters.selected_energy as f64 / 10000.0,
                energy_rate: trade.parameters.energy_rate as f64 / 10000.0,
                trade_uuid: h256_to_string(trade.parameters.trade_uuid)
            },
        })
    }
    deserialized
}