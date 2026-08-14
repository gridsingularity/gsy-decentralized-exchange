use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::utils::H256;
use subxt::config::{Hasher, substrate::BlakeTwo256};
use crate::db_api_schema::orders::{DbOffer, DbBid};


/// Trade status.
///
/// A trade is inserted as `Settled` and stays there until the execution engine has judged it and
/// gsy-node has reported the verdict, at which point the event listener moves it to `Executed` or
/// `Penalized`. New variants must be appended, never inserted: the SCALE discriminants are part of
/// the wire format.
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub enum TradeStatus {
    /// Matched and recorded on-chain, not yet evaluated for delivery.
    Settled,
    /// Evaluated by the execution engine with no penalty incurred.
    Executed,
    /// Evaluated by the execution engine and penalized on-chain.
    Penalized,
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
    pub offer: DbOffer,
    pub offer_hash: String,
    pub bid: DbBid,
    pub bid_hash: String,
    pub residual_offer: Option<DbOffer>,
    pub residual_bid: Option<DbBid>,
    pub parameters: TradeParameters,
}

impl TradeSchema {
    pub fn hash(&self) -> H256 {
        BlakeTwo256.hash_of(self)
    }

    pub fn eq(&self, other: &Self) -> bool {
        self._id == other._id
    }

}

/// A trade enriched with the human-readable asset names for the seller and
/// buyer sides, resolved from the topology `area_hash` -> `name` mapping.
/// The original `TradeSchema` fields (including the `seller`/`buyer` account
/// ids) are flattened in at the top level and left untouched.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TradeCanonicalSchema {
    #[serde(flatten)]
    pub trade: TradeSchema,
    pub seller_name: Option<String>,
    pub buyer_name: Option<String>,
}
