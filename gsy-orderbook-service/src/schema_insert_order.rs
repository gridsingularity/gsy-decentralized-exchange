use serde::{Deserialize, Serialize};
use codec::{Encode, Decode};

#[derive(Serialize, Deserialize, Encode, Decode)]
#[serde(tag = "type", content = "data")]
pub enum Order<AccountId32> {
    Bid(Bid<AccountId32>),
    Offer(Offer<AccountId32>),
}

#[derive(Serialize, Deserialize, Debug, Decode)]
pub struct OrderComponent{
    pub area_uuid: u64,
    pub market_uuid: u64,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64
}

#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct OrderSchema<AccountId32, Hash> {
    pub _id: Hash,
    pub status: OrderStatus,
    pub order: Order<AccountId32>,
}

/// Order status
#[derive(Serialize, Deserialize, Encode, Decode)]
pub enum OrderStatus {
    Open,
    Executed,
    Expired,
    Deleted,
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Serialize, Deserialize, Decode)]
pub struct Bid<AccountId32> {
    pub buyer: AccountId32,
    pub nonce: u32,
    pub bid_component: OrderComponent,
}

#[derive(Serialize, Deserialize, Debug, Decode)]
pub struct Offer<AccountId32>{
    pub seller: AccountId32,
    pub nonce: u32,
    pub offer_component: OrderComponent,
}
