use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};
use crate::db_api_schema::orders::{Offer, Bid};


/// Trade status
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum TradeStatus {
    Settled,
    Executed,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeParameters {
    pub selected_energy: f64,
    pub energy_rate: f64,
    pub trade_uuid: String,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TradeSchema {
    pub _id: String,
    pub status: TradeStatus,
    pub seller: String,
    pub buyer: String,
    pub market_id: String,
    pub time_slot: u64,
    pub trade_uuid: String,
    pub creation_time: u64,
    pub offer: Offer,
    pub offer_hash: String,
    pub bid: Bid,
    pub bid_hash: String,
    pub residual_offer: Option<Offer>,
    pub residual_bid: Option<Bid>,
    pub parameters: TradeParameters,
}

impl TradeSchema {
    pub fn hash(&self) -> H256 {
        BlakeTwo256::hash_of(self)
    }

    pub fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }

}
