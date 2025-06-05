use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;

#[derive(Serialize, Deserialize, Encode, Decode, Debug, Clone)]
pub struct BidOfferMatch<AccountId32> {
    /// The market ID
    pub market_id: u8,
    /// The time slot
    pub time_slot: u64,
    /// The bid
    pub bid: Bid<AccountId32>,
    /// The offer
    pub offer: Offer<AccountId32>,
    /// The residual offer (if any)
    pub residual_offer: Option<Offer<AccountId32>>,
    /// The residual bid (if any)
    pub residual_bid: Option<Bid<AccountId32>>,
    /// The amount of energy that is traded.
    pub selected_energy: u64,
    /// The price of the selected energy.
    pub energy_rate: u64,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug, Clone)]
pub enum Order<AccountId32> {
    Bid(Bid<AccountId32>),
    Offer(Offer<AccountId32>),
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct OrderComponent{
    pub area_uuid: H256,
    pub market_id: H256,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Bid<AccountId32> {
    pub buyer: AccountId32,
    pub nonce: u32,
    pub bid_component: OrderComponent,
}


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct Offer<AccountId32>{
    pub seller: AccountId32,
    pub nonce: u32,
    pub offer_component: OrderComponent,
}