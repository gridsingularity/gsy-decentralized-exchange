use crate::algorithms::PayAsBid;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use subxt::ext::sp_core::H256;
use subxt::utils::AccountId32;

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub enum EnergyType {
    Clean,
    Battery,
    FossilFuel,
    Import,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Requirements {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Attributes {
    pub trading_partner_id: Option<AccountId32>,
    pub energy_type: EnergyType,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct OrderComponent {
    pub area_uuid: H256,
    pub market_id: H256,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Bid {
    pub buyer: AccountId32,
    pub nonce: u32,
    pub bid_component: OrderComponent,
    pub requirements: Option<Requirements>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Offer {
    pub seller: AccountId32,
    pub nonce: u32,
    pub offer_component: OrderComponent,
    pub attributes: Option<Attributes>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Order {
    Bid(Bid),
    Offer(Offer),
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct BidOfferMatch {
    pub market_id: H256,
    pub time_slot: u64,
    pub bid: Bid,
    pub offer: Offer,
    pub residual_bid: Option<Bid>,
    pub residual_offer: Option<Offer>,
    pub selected_energy: u64,
    pub energy_rate: u64,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingData {
    pub bids: Vec<Bid>,
    pub offers: Vec<Offer>,
    pub market_id: H256,
}

impl MatchingData {
    fn match_preferences(&self) -> (Vec<BidOfferMatch>, Vec<Bid>, Vec<Offer>) {
        let mut matches = Vec::new();

        type OrderKey = ([u8; 32], u32);

        let mut bid_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();
        let mut offer_matched_amounts: HashMap<OrderKey, u64> = HashMap::new();

        let (preference_bids, _non_preference_bids): (Vec<&Bid>, Vec<&Bid>) =
            self.bids.iter().partition(|b| {
                b.requirements
                    .as_ref()
                    .and_then(|r| r.trading_partner_id.as_ref())
                    .is_some()
            });

        for bid in preference_bids {
            let req = bid.requirements.as_ref().unwrap();
            let partner_id = req.trading_partner_id.as_ref().unwrap();
            let bid_key = (bid.buyer.0, bid.nonce);

            let partner_offers: Vec<&Offer> = self
                .offers
                .iter()
                .filter(|o| &o.seller == partner_id)
                .collect();

            for offer in partner_offers {
                let offer_key = (offer.seller.0, offer.nonce);

                let bid_amount_used = *bid_matched_amounts.get(&bid_key).unwrap_or(&0);
                let offer_amount_used = *offer_matched_amounts.get(&offer_key).unwrap_or(&0);

                let bid_energy_needed = bid.bid_component.energy.saturating_sub(bid_amount_used);
                let offer_energy_available = offer
                    .offer_component
                    .energy
                    .saturating_sub(offer_amount_used);

                let selected_energy = bid_energy_needed.min(offer_energy_available);

                if selected_energy > 0 {
                    let trade_rate = req
                        .preferred_energy_rate
                        .unwrap_or(bid.bid_component.energy_rate);

                    matches.push(BidOfferMatch {
                        market_id: offer.offer_component.market_id,
                        time_slot: offer.offer_component.time_slot,
                        bid: bid.clone(),
                        offer: offer.clone(),
                        residual_bid: None,
                        residual_offer: None,
                        selected_energy,
                        energy_rate: trade_rate,
                    });

                    *bid_matched_amounts.entry(bid_key.clone()).or_insert(0) += selected_energy;
                    *offer_matched_amounts.entry(offer_key).or_insert(0) += selected_energy;

                    if bid
                        .bid_component
                        .energy
                        .saturating_sub(*bid_matched_amounts.get(&bid_key).unwrap_or(&0))
                        == 0
                    {
                        break;
                    }
                }
            }
        }

        let mut remaining_bids = Vec::new();

        for bid in self.bids.iter() {
            let has_reqs = bid
                .requirements
                .as_ref()
                .and_then(|r| r.trading_partner_id.as_ref())
                .is_some();

            if has_reqs {
                let bid_key = (bid.buyer.0, bid.nonce);
                let matched_amount = *bid_matched_amounts.get(&bid_key).unwrap_or(&0);

                if bid.bid_component.energy > matched_amount {
                    let mut residual_bid = bid.clone();
                    residual_bid.bid_component.energy -= matched_amount;
                    if matched_amount > 0 {
                        residual_bid.nonce = residual_bid.nonce.wrapping_add(1);
                    }
                    remaining_bids.push(residual_bid);
                }
            } else {
                remaining_bids.push(bid.clone());
            }
        }

        let mut remaining_offers = Vec::new();
        for offer in self.offers.iter() {
            let offer_key = (offer.seller.0, offer.nonce);
            let matched_amount = *offer_matched_amounts.get(&offer_key).unwrap_or(&0);

            if offer.offer_component.energy > matched_amount {
                let mut residual_offer = offer.clone();
                residual_offer.offer_component.energy -= matched_amount;
                if matched_amount > 0 {
                    residual_offer.nonce = residual_offer.nonce.wrapping_add(1);
                }
                remaining_offers.push(residual_offer);
            }
        }

        (matches, remaining_bids, remaining_offers)
    }

    fn match_standard(&self, mut bids: Vec<Bid>, mut offers: Vec<Offer>) -> Vec<BidOfferMatch> {
        let mut matches = Vec::new();

        bids.sort_by(|a, b| {
            b.bid_component
                .energy_rate
                .cmp(&a.bid_component.energy_rate)
        });
        offers.sort_by(|a, b| {
            a.offer_component
                .energy_rate
                .cmp(&b.offer_component.energy_rate)
        });

        type OrderKey = ([u8; 32], u32);
        let mut available_energy_bid: HashMap<OrderKey, u64> = HashMap::new();
        let mut available_energy_offer: HashMap<OrderKey, u64> = HashMap::new();

        for b in &bids {
            available_energy_bid.insert((b.buyer.0, b.nonce), b.bid_component.energy);
        }
        for o in &offers {
            available_energy_offer.insert((o.seller.0, o.nonce), o.offer_component.energy);
        }

        for offer in &mut offers {
            for bid in &mut bids {
                if offer.offer_component.area_uuid == bid.bid_component.area_uuid {
                    continue;
                }

                if offer.offer_component.energy_rate > bid.bid_component.energy_rate {
                    continue;
                }

                let bid_key = (bid.buyer.0, bid.nonce);
                let offer_key = (offer.seller.0, offer.nonce);

                let offer_energy = *available_energy_offer.get(&offer_key).unwrap_or(&0);
                let bid_energy = *available_energy_bid.get(&bid_key).unwrap_or(&0);

                if offer_energy == 0 || bid_energy == 0 {
                    continue;
                }

                let selected_energy = offer_energy.min(bid_energy);

                available_energy_bid.insert(bid_key.clone(), bid_energy - selected_energy);
                available_energy_offer.insert(offer_key.clone(), offer_energy - selected_energy);

                let residual_bid = if bid_energy > selected_energy {
                    Some(Bid {
                        nonce: bid.nonce.wrapping_add(1),
                        bid_component: OrderComponent {
                            energy: bid_energy - selected_energy,
                            ..bid.bid_component.clone()
                        },
                        ..bid.clone()
                    })
                } else {
                    None
                };

                let residual_offer = if offer_energy > selected_energy {
                    Some(Offer {
                        nonce: offer.nonce.wrapping_add(1),
                        offer_component: OrderComponent {
                            energy: offer_energy - selected_energy,
                            ..offer.offer_component.clone()
                        },
                        ..offer.clone()
                    })
                } else {
                    None
                };

                let new_bid_offer_match = BidOfferMatch {
                    market_id: offer.offer_component.market_id,
                    time_slot: offer.offer_component.time_slot,
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid,
                    residual_offer,
                    selected_energy,
                    energy_rate: bid.bid_component.energy_rate,
                };

                matches.push(new_bid_offer_match);
            }
        }
        matches
    }
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut all_matches = Vec::new();

        let (pref_matches, remaining_bids, remaining_offers) = self.match_preferences();
        all_matches.extend(pref_matches);

        let standard_matches = self.match_standard(remaining_bids, remaining_offers);
        all_matches.extend(standard_matches);

        all_matches
    }
}
