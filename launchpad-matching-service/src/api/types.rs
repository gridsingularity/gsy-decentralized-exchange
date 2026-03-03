use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use serde_json;
use subxt::utils::H256;
use subxt::config::{Hasher, substrate::BlakeTwo256};

const EPS: f64 = 0.000001;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbBidOfferMatch {
    pub market_id: String,
    pub time_slot: u64,
    pub bid: DbBid,
    pub offer: DbOffer,
    pub residual_bid: Option<DbBid>,
    pub residual_offer: Option<DbOffer>,
    pub selected_energy: f64,
    pub energy_rate: f64,
}


#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DbMatchingData {
    pub bids: Vec<DbBid>,
    pub offers: Vec<DbOffer>,
    pub market_id: String,
}

impl PayAsBid for DbMatchingData {
    type Output = DbBidOfferMatch;

    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut bid_offer_pairs = Vec::new();

        let mut bids = self.bids.clone();
        let mut offers = self.offers.clone();

        bids.sort_by(|a, b| b.bid_component.energy_rate.partial_cmp(&a.bid_component.energy_rate).unwrap());
        offers.sort_by(|a, b| a.offer_component.energy_rate.partial_cmp(&b.offer_component.energy_rate).unwrap());

        let mut available_order_energy: HashMap<H256, f64> = HashMap::new();

        for offer in &mut offers {
            for bid in &mut bids {
                if offer.offer_component.area_uuid == bid.bid_component.area_uuid
                    || offer.offer_component.energy < EPS
                    || bid.bid_component.energy < EPS
                {
                    continue;
                }

                if offer.offer_component.energy_rate > bid.bid_component.energy_rate {
                    continue;
                }

                let serialized_bid = serde_json::to_string(bid).expect(
                    "Failed to serialize DbBid to JSON String");
                let serialized_offer = serde_json::to_string(offer).expect(
                    "Failed to serialize DbOffer to JSON String");
                let bid_id = BlakeTwo256.hash_of(&serialized_bid);
                let offer_id = BlakeTwo256.hash_of(&serialized_offer);

                let offer_energy =
                    *available_order_energy.entry(offer_id.clone()).or_insert(offer.offer_component.energy);
                let bid_energy =
                    *available_order_energy.entry(bid_id.clone()).or_insert(bid.bid_component.energy);

                let selected_energy = offer_energy.min(bid_energy);

                if selected_energy < EPS {
                    continue;
                }

                available_order_energy.insert(bid_id, bid_energy - selected_energy);
                available_order_energy.insert(offer_id, offer_energy - selected_energy);

                let residual_bid = if bid_energy > selected_energy {
                    Some(DbBid {
                        nonce: bid.nonce.wrapping_add(1),
                        bid_component: DbOrderComponent {
                            energy: bid_energy - selected_energy,
                            ..bid.bid_component.clone()
                        },
                        ..bid.clone()
                    })
                } else {
                    None
                };

                let residual_offer = if offer_energy > selected_energy {
                    Some(DbOffer {
                        nonce: offer.nonce.wrapping_add(1),
                        offer_component: DbOrderComponent {
                            energy: offer_energy - selected_energy,
                            ..offer.offer_component.clone()
                        },
                        ..offer.clone()
                    })
                } else {
                    None
                };

                let new_bid_offer_match = DbBidOfferMatch {
                    market_id: offer.offer_component.market_id.clone(),
                    time_slot: offer.offer_component.time_slot,
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid,
                    residual_offer,
                    selected_energy,
                    energy_rate: bid.bid_component.energy_rate,
                };

                bid_offer_pairs.push(new_bid_offer_match);
            }
        }
        bid_offer_pairs
    }
}
