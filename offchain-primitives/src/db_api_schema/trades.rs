use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};
use crate::db_api_schema::orders::{Offer, Bid};


/// Trade status
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum TradeStatus {
    Open,
    Executed,
    Expired,
    Deleted,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeParameters {
    selected_energy: u64,
    energy_rate: u64,
    trade_uuid: H256,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeSchema {
    pub _id: H256,
    pub status: TradeStatus,
    seller: String,
    buyer: String,
    market_id: u64,
    time_slot: u64,
    trade_uuid: H256,
    creation_time: u64,
    offer: Offer,
    offer_hash: H256,
    bid: Bid,
    bid_hash: H256,
    residual_offer: Offer,
    residual_bid: Bid,
    parameters: TradeParameters,
}

impl TradeSchema {
    pub fn hash(&self) -> H256 {
        BlakeTwo256::hash_of(self)
    }
}
