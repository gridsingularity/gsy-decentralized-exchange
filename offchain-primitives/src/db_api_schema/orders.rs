use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};


#[derive(Serialize, Deserialize, Debug, Encode, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Order {
    Bid(Bid),
    Offer(Offer),
}

impl Order {
    pub fn hash(&self) -> H256 {
        BlakeTwo256::hash_of(self)
    }
}
/// Order component struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct OrderComponent {
    pub area_uuid: String,
    pub market_uuid: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: f64,
    pub energy_rate: f64
}

#[derive(Serialize, Deserialize, Debug, Encode, Clone, PartialEq)]
pub struct OrderSchema {
    pub _id: H256,
    pub status: OrderStatus,
    pub order: Order,
}

impl From<Order> for OrderSchema {
    fn from(order: Order) -> Self {
        OrderSchema {
            _id: order.hash(),
            status: Default::default(),
            order,
        }
    }
}

/// Order status
#[derive(Serialize, Deserialize, Debug, Encode, Clone, PartialEq)]
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

/// Bid order struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct Bid {
    pub buyer: String,
    pub nonce: u32,
    pub bid_component: OrderComponent,
}

/// Offer (Ask) order struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct Offer {
    pub seller: String,
    pub nonce: u32,
    pub offer_component: OrderComponent,
}
