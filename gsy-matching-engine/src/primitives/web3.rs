use crate::algorithms::PayAsBid;
use crate::algorithms::PayAsClear;
use crate::algorithms::pay_as_clear::{get_clearing_point_web3, create_bid_offer_matches_web3, ClearingInfo};
use std::cmp::Ordering;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};
use gsy_offchain_primitives::service_to_node_schema::orders::{OrderComponent, Bid, Offer};

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

impl PayAsClear for MatchingData {
    type Output = BidOfferMatch;
    type BidType = Bid;
    type OfferType = Offer;

    fn pay_as_clear(&mut self) -> Vec<Self::Output> {
        if self.bids.is_empty() || self.offers.is_empty() {
            return Vec::new();
        }

        let mut sorted_bids = self.bids.clone();
        let mut sorted_offers = self.offers.clone();

        // Sort bids by energy_rate descending
        sorted_bids.sort_by(|a, b| {
            b.bid_component.energy_rate.cmp(&a.bid_component.energy_rate)
        });
        // Sort offers by energy_rate ascending
        sorted_offers.sort_by(|a, b| {
            a.offer_component.energy_rate.cmp(&b.offer_component.energy_rate)
        });
        
        // Use time_slot from the first bid
        let time_slot = self.bids.first().map_or(0, |b| b.bid_component.time_slot);


        if let Some(clearing_info) = get_clearing_point_web3(&sorted_bids, &sorted_offers) {
            if clearing_info.energy > 0 {
                // Filter bids and offers that are not eligible at the clearing price
                let eligible_bids: Vec<Bid> = sorted_bids
                    .into_iter()
                    .filter(|b| b.bid_component.energy_rate >= clearing_info.rate)
                    .collect();
                
                let eligible_offers: Vec<Offer> = sorted_offers
                    .into_iter()
                    .filter(|o| o.offer_component.energy_rate <= clearing_info.rate)
                    .collect();

                if eligible_bids.is_empty() || eligible_offers.is_empty() {
                    return Vec::new();
                }
                
                return create_bid_offer_matches_web3(
                    &eligible_bids,
                    &eligible_offers,
                    &clearing_info,
                    self.market_id,
                    time_slot,
                );
            }
        }
        Vec::new()
    }
}
