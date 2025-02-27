use codec::{Encode, Decode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};


#[derive(Serialize, Deserialize, Debug, Encode, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Order {
    Bid(DbBid),
    Offer(DbOffer),
}

impl Order {
    pub fn hash(&self) -> H256 {
        BlakeTwo256::hash_of(self)
    }
}
/// Order component struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct DbOrderComponent {
    pub area_uuid: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: f64,
    pub energy_rate: f64
}

#[derive(Serialize, Deserialize, Debug, Encode, Clone, PartialEq)]
pub struct DbOrderSchema {
    pub _id: String,
    pub status: OrderStatus,
    pub order: Order,
}

impl From<Order> for DbOrderSchema {
    fn from(order: Order) -> Self {
        DbOrderSchema {
            _id: order.hash().to_string(),
            status: Default::default(),
            order,
        }
    }
}

/// Order status
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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
pub struct DbBid {
    pub buyer: String,
    pub nonce: u32,
    pub bid_component: DbOrderComponent,
}

/// Offer (Ask) order struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct DbOffer {
    pub seller: String,
    pub nonce: u32,
    pub offer_component: DbOrderComponent,
}

