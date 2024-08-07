use crate::algorithms::PayAsBid;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct BidOfferMatch {
    pub market_id: u8,
    pub time_slot: u64,
    pub bid: Bid,
    pub offer: Offer,
    pub residual_offer: Option<Offer>,
    pub residual_bid: Option<Bid>,
    pub selected_energy: u64,
    pub energy_rate: u64,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingData {
    pub bids: Vec<Bid>,
    pub offers: Vec<Offer>,
    pub market_id: u8,
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut bid_offer_pairs = Vec::new();

        let mut bids = self.bids.clone();
        let mut offers = self.offers.clone();

        bids.sort_by(|a, b| {
            b.bid_component
                .energy_rate
                .partial_cmp(&a.bid_component.energy_rate)
                .unwrap()
        });
        offers.sort_by(|a, b| {
            b.offer_component
                .energy_rate
                .partial_cmp(&a.offer_component.energy_rate)
                .unwrap()
        });

        let mut available_order_energy: HashMap<H256, u64> = HashMap::new();
        for offer in &mut offers {
            for bid in &mut bids {
                if offer.offer_component.area_uuid == bid.bid_component.area_uuid
                    || offer.offer_component.energy == 0
                    || bid.bid_component.energy == 0
                {
                    continue;
                }

                if offer
                    .offer_component
                    .energy_rate
                    .checked_sub(bid.bid_component.energy_rate)
                    .unwrap_or(0)
                    > 0
                {
                    continue;
                }

                let bid_id =
                    BlakeTwo256::hash_of(&format!("{}:{}", bid.buyer.clone(), bid.nonce.clone()));
                let offer_id = BlakeTwo256::hash_of(&format!(
                    "{}:{}",
                    offer.seller.clone(),
                    offer.nonce.clone()
                ));
                if !available_order_energy.contains_key(&bid_id) {
                    available_order_energy
                        .insert(bid_id.clone(), bid.bid_component.energy)
                        .unwrap_or(0);
                }
                if !available_order_energy.contains_key(&offer_id) {
                    available_order_energy
                        .insert(offer_id.clone(), offer.offer_component.energy)
                        .unwrap_or(0);
                }

                let offer_energy = available_order_energy.get(&offer_id).unwrap().clone();
                let bid_energy = available_order_energy.get(&bid_id).unwrap().clone();

                let selected_energy = offer_energy.min(bid_energy);

                if selected_energy <= 0 {
                    continue;
                }

                available_order_energy.insert(
                    bid_id.clone(),
                    bid_energy.checked_sub(selected_energy).unwrap(),
                );
                available_order_energy.insert(
                    offer_id.clone(),
                    offer_energy.checked_sub(selected_energy).unwrap(),
                );

                let residual_bid_struct: Option<Bid> =
                    if available_order_energy.get(&bid_id).unwrap() > &0u64 {
                        Some(Bid {
                            nonce: bid.nonce.clone().checked_add(1).unwrap(),
                            bid_component: OrderComponent {
                                energy: available_order_energy.get(&bid_id).unwrap().clone(),
                                ..bid.bid_component.clone()
                            },
                            ..bid.clone()
                        })
                    } else {
                        None
                    };

                let residual_offer_struct: Option<Offer> =
                    if available_order_energy.get(&offer_id).unwrap() > &0u64 {
                        Some(Offer {
                            nonce: offer.nonce.clone().checked_add(1).unwrap(),
                            offer_component: OrderComponent {
                                energy: available_order_energy.get(&offer_id).unwrap().clone(),
                                ..offer.offer_component.clone()
                            },
                            ..offer.clone()
                        })
                    } else {
                        None
                    };

                let new_bid_offer_match = BidOfferMatch {
                    market_id: self.market_id.clone(),
                    time_slot: offer.offer_component.time_slot,
                    bid: bid.clone(),
                    selected_energy,
                    offer: offer.clone(),
                    residual_offer: residual_offer_struct,
                    residual_bid: residual_bid_struct,
                    energy_rate: bid.bid_component.energy_rate,
                };
                bid_offer_pairs.push(new_bid_offer_match);

                bid.bid_component.energy = available_order_energy.get(&bid_id).unwrap().clone();
                offer.offer_component.energy =
                    available_order_energy.get(&offer_id).unwrap().clone();

                if let Some(offer_residual_energy) = available_order_energy.get(&offer_id) {
                    if *offer_residual_energy <= 0 {
                        break;
                    }
                }
            }
        }
        bid_offer_pairs
    }
}
