use crate::algorithms::PayAsClear;
use crate::algorithms::pay_as_clear::{get_clearing_point_web2, create_bid_offer_matches_web2, ClearingInfo as PayAsClearClearingInfo};
use std::cmp::Ordering;
use serde::{Serialize, Deserialize, Serializer};
use std::collections::HashMap;
use chrono::{NaiveDateTime};
use crate::algorithms::PayAsBid;

const FLOATING_POINT_TOLERANCE: f32 = 0.00001;
const MATCH_FLOATING_POINT_TOLERANCE_F32_INTERNAL: f32 = 1e-8;

pub fn serialize_datetime<S>(
    datetime: &Option<NaiveDateTime>,
    serializer: S
) -> Result<S::Ok, S::Error>
    where
        S: Serializer {
    const FORMAT: &'static str = "%Y-%m-%dT%H:%M";
    match datetime {
        Some(datetime) => {
            let s = format!("{}", datetime.format(FORMAT));
            serializer.serialize_str(&s)
        },
        None => serializer.serialize_none()
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Bid {
    pub r#type: String,
    pub id: String,
    pub energy: f32,
    pub energy_rate: f32,
    pub original_price: f32,
    pub requirements: Option<String>,
    pub buyer_origin: String,
    pub buyer_origin_id: String,
    pub buyer_id: String,
    pub buyer: String,
    #[serde(serialize_with = "serialize_datetime")]
    pub time_slot: Option<NaiveDateTime>,
    pub creation_time: Option<NaiveDateTime>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Offer {
    pub r#type: String,
    pub id: String,
    pub energy: f32,
    pub energy_rate: f32,
    pub original_price: f32,
    pub requirements: Option<String>,
    pub seller_origin: String,
    pub seller_origin_id: String,
    pub seller_id: String,
    pub seller: String,
    #[serde(serialize_with = "serialize_datetime")]
    pub time_slot: Option<NaiveDateTime>,
    pub creation_time: Option<NaiveDateTime>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BidOfferMatch {
    pub market_id: String,
    #[serde(serialize_with = "serialize_datetime")]
    pub time_slot: Option<NaiveDateTime>,
    pub bid: Bid,
    pub selected_energy: f32,
    pub offer: Offer,
    pub trade_rate: f32,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingData {
    pub bids: Vec<Bid>,
    pub offers: Vec<Offer>,
    pub market_id: String
}

impl PayAsBid for MatchingData {
    type Output = BidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut bid_offer_pairs = Vec::new();

        self.bids.sort_by(|a, b| b.energy_rate.partial_cmp(&a.energy_rate).unwrap());
        self.offers.sort_by(|a, b| b.energy_rate.partial_cmp(&a.energy_rate).unwrap());

        let mut available_order_energy: HashMap<String,f32> = HashMap::new();
        for offer in self.offers.clone() {
            for bid in self.bids.clone() {
                if offer.seller == bid.buyer {
                    continue;
                }

                if offer.energy_rate - bid.energy_rate > FLOATING_POINT_TOLERANCE {
                    continue;
                }

                if !available_order_energy.contains_key(bid.id.as_str()) {
                    available_order_energy.insert(bid.id.clone(), bid.energy).unwrap_or(0.0);
                }
                if !available_order_energy.contains_key(offer.id.as_str()) {
                    available_order_energy.insert(offer.id.clone(), offer.energy).unwrap_or(0.0);
                }

                let offer_energy = available_order_energy.get(
                    offer.id.as_str()).unwrap().clone();
                let bid_energy = available_order_energy.get(
                    bid.id.as_str()).unwrap().clone();

                let selected_energy = offer_energy.min(bid_energy);

                if selected_energy <= FLOATING_POINT_TOLERANCE {
                    continue;
                }

                available_order_energy.insert(bid.id.clone(), bid_energy - selected_energy);
                available_order_energy.insert(offer.id.clone(),
                                              offer_energy - selected_energy);

                assert!(available_order_energy.values().all(
                    |energy| *energy >= -FLOATING_POINT_TOLERANCE));

                let new_bid_offer_match = BidOfferMatch {
                        market_id: self.market_id.clone(),
                        time_slot: offer.time_slot,
                        bid: bid.clone(),
                        selected_energy,
                        trade_rate: bid.energy_rate,
                        offer: offer.clone(),
                };
                bid_offer_pairs.push(new_bid_offer_match);

                if let Some(offer_residual_energy) = available_order_energy.get(
                    offer.id.as_str()) {
                    if *offer_residual_energy <= FLOATING_POINT_TOLERANCE {
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

        sorted_bids.sort_by(|a, b| {
            b.energy_rate.partial_cmp(&a.energy_rate).unwrap_or(Ordering::Equal)
        });
        sorted_offers.sort_by(|a, b| {
            a.energy_rate.partial_cmp(&b.energy_rate).unwrap_or(Ordering::Equal)
        });

        if let Some(clearing_info) = get_clearing_point_web2(&sorted_bids, &sorted_offers) {
             if clearing_info.energy > MATCH_FLOATING_POINT_TOLERANCE_F32_INTERNAL {
                let eligible_bids: Vec<Bid> = sorted_bids
                    .into_iter()
                    .filter(|b| b.energy_rate >= clearing_info.rate - MATCH_FLOATING_POINT_TOLERANCE_F32_INTERNAL)
                    .collect();
                
                let eligible_offers: Vec<Offer> = sorted_offers
                    .into_iter()
                    .filter(|o| o.energy_rate <= clearing_info.rate + MATCH_FLOATING_POINT_TOLERANCE_F32_INTERNAL)
                    .collect();

                if eligible_bids.is_empty() || eligible_offers.is_empty() {
                    return Vec::new();
                }

                return create_bid_offer_matches_web2(
                    &eligible_bids,
                    &eligible_offers,
                    &clearing_info,
                    self.market_id.clone(),
                );
            }
        }
        Vec::new()
    }
}