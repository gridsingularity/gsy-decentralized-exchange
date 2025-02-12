use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};


#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
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
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct OrderComponent {
    pub area_uuid: u64,
    pub market_uuid: u64,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

impl From<OrderSchema> for Order {
    fn from(order: OrderSchema) -> Self {
        order.order
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
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Bid {
    pub buyer: String,
    pub nonce: u32,
    pub bid_component: OrderComponent,
}

impl From<Order> for Bid {
    fn from(order: Order) -> Self {
        match order {
            Order::Bid(bid) => bid,
            _ => panic!("Expected Order::Bid"),
        }
    }
}

impl From<OrderSchema> for Bid {
    fn from(order: OrderSchema) -> Self {
        match order.order {
            Order::Bid(bid) => bid,
            _ => panic!("Expected Order::Bid"),
        }
    }
}

/// Offer (Ask) order struct
#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Offer {
    pub seller: String,
    pub nonce: u32,
    pub offer_component: OrderComponent,
}

impl From<Order> for Offer {
    fn from(order: Order) -> Self {
        match order {
            Order::Offer(offer) => offer,
            _ => panic!("Expected Order::Offer"),
        }
    }
}

impl From<OrderSchema> for Offer {
    fn from(order: OrderSchema) -> Self {
        match order.order {
            Order::Offer(offer) => offer,
            _ => panic!("Expected Order::Offer"),
        }
    }
}