use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema,
};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const EPS: f64 = 0.000001;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrdersToMatch {
    pub user_id: String,
    pub orders: Vec<DbOrderSchema>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbBidOfferMatch {
    /// Unique id of the match (hex ObjectId, time-ordered), assigned when the
    /// match is created.
    pub match_id: String,
    pub user_id: String,
    pub market_id: String,
    pub time_slot: u64,
    /// `_id` of the submitted `DbOrderSchema` the bid came from.
    pub bid_order_id: String,
    /// `_id` of the submitted `DbOrderSchema` the offer came from.
    pub offer_order_id: String,
    pub bid: DbBid,
    pub offer: DbOffer,
    pub residual_bid: Option<DbBid>,
    pub residual_offer: Option<DbOffer>,
    pub selected_energy: f64,
    pub energy_rate: f64,
}

/// A bid together with the `_id` of the submitted order it came from.
#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingBid {
    pub order_id: String,
    pub bid: DbBid,
}

/// An offer together with the `_id` of the submitted order it came from.
#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingOffer {
    pub order_id: String,
    pub offer: DbOffer,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DbMatchingData {
    pub bids: Vec<MatchingBid>,
    pub offers: Vec<MatchingOffer>,
    pub market_id: String,
    pub user_id: String,
}

/// Index of the first live (not exhausted) bid at or after `index`, with
/// path halving. `next[i] == i` while bid `i` is live; an exhausted bid
/// points past itself. `next` has one extra slot (`next[n] == n`) as the end
/// sentinel.
fn next_live(next: &mut [usize], mut index: usize) -> usize {
    while next[index] != index {
        let skip = next[next[index]];
        next[index] = skip;
        index = skip;
    }
    index
}

impl PayAsBid for DbMatchingData {
    type Output = DbBidOfferMatch;

    /// Pay-As-Bid on the bids and offers of one (market, time slot).
    ///
    /// Offers are visited by ascending `energy_rate` and, for each offer, the
    /// bids by descending `energy_rate` (both stable sorts, so equal rates
    /// keep the submitted order). A pair matches when the offer's rate is at
    /// most the bid's, both orders still have at least `EPS` kWh left and
    /// their `area_uuid`s differ; the matched energy is the smaller remaining
    /// energy and the price is the bid's rate.
    ///
    /// The remaining energy is tracked per submitted order (by its position,
    /// which identifies it like its `_id`). Exhausted bids are skipped
    /// without being visited again, and because the bids are sorted by
    /// descending rate the inner loop stops at the first bid priced below
    /// the offer. The matches and their values are the same as visiting every
    /// pair in that order.
    fn pay_as_bid(&mut self) -> Vec<Self::Output> {
        let mut bid_offer_pairs = Vec::new();

        let bid_rate = |i: usize| self.bids[i].bid.bid_component.energy_rate;
        let offer_rate_of = |i: usize| self.offers[i].offer.offer_component.energy_rate;

        // Bid positions in descending order of energy rate (b is compared
        // with a). `sort_by` is stable.
        let mut bids: Vec<usize> = (0..self.bids.len()).collect();
        bids.sort_by(|&a, &b| bid_rate(b).partial_cmp(&bid_rate(a)).unwrap());
        // Offer positions in ascending order of energy rate (a is compared
        // with b).
        let mut offers: Vec<usize> = (0..self.offers.len()).collect();
        offers.sort_by(|&a, &b| offer_rate_of(a).partial_cmp(&offer_rate_of(b)).unwrap());

        // Remaining energy per bid, indexed by the bid's position in `bids`.
        let mut bid_remaining: Vec<f64> = bids
            .iter()
            .map(|&i| self.bids[i].bid.bid_component.energy)
            .collect();
        // Rate and area of the bids in sorted order, contiguous for the inner
        // loop. Areas are interned so they compare as integers.
        let mut area_ids: HashMap<&str, u32> = HashMap::new();
        let bid_keys: Vec<(f64, u32)> = bids
            .iter()
            .map(|&i| {
                let component = &self.bids[i].bid.bid_component;
                let next_id = area_ids.len() as u32;
                let area = *area_ids
                    .entry(component.area_uuid.as_str())
                    .or_insert(next_id);
                (component.energy_rate, area)
            })
            .collect();
        // Skip list over `bids`: an order with less than EPS left (from the
        // start or after a match) can never match again.
        let mut next: Vec<usize> = (0..=bids.len()).collect();
        for (position, remaining) in bid_remaining.iter().enumerate() {
            if *remaining < EPS {
                next[position] = position + 1;
            }
        }

        for &offer_index in &offers {
            let matching_offer = &self.offers[offer_index];
            let offer = &matching_offer.offer;
            let mut offer_energy = offer.offer_component.energy;
            if offer_energy < EPS {
                continue;
            }
            let offer_rate = offer.offer_component.energy_rate;
            // An area no bid has gets an id no bid has either.
            let offer_area = area_ids
                .get(offer.offer_component.area_uuid.as_str())
                .copied()
                .unwrap_or(u32::MAX);

            let mut position = next_live(&mut next, 0);
            if position == bids.len() {
                // Every bid is exhausted.
                break;
            }
            if offer_rate > bid_keys[position].0 {
                // Priced above the best live bid; offers come in ascending
                // order, so no later offer can match either.
                break;
            }

            while position < bids.len() {
                let (bid_rate, bid_area) = bid_keys[position];
                // Bids are sorted by descending rate: every later bid is
                // priced below this offer as well.
                if offer_rate > bid_rate {
                    break;
                }
                if offer_area == bid_area {
                    position = next_live(&mut next, position + 1);
                    continue;
                }
                let matching_bid = &self.bids[bids[position]];
                let bid = &matching_bid.bid;

                // Same arithmetic, in the same order, as the unoptimized
                // version, so the energies are bit for bit identical.
                let bid_energy = bid_remaining[position];
                let current_offer_energy = offer_energy;
                let selected_energy = current_offer_energy.min(bid_energy);

                bid_remaining[position] = bid_energy - selected_energy;
                offer_energy = current_offer_energy - selected_energy;

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

                let residual_offer = if current_offer_energy > selected_energy {
                    Some(DbOffer {
                        nonce: offer.nonce.wrapping_add(1),
                        offer_component: DbOrderComponent {
                            energy: current_offer_energy - selected_energy,
                            ..offer.offer_component.clone()
                        },
                        ..offer.clone()
                    })
                } else {
                    None
                };

                bid_offer_pairs.push(DbBidOfferMatch {
                    match_id: ObjectId::new().to_hex(),
                    user_id: self.user_id.clone(),
                    market_id: offer.offer_component.market_id.clone(),
                    time_slot: offer.offer_component.time_slot,
                    bid_order_id: matching_bid.order_id.clone(),
                    offer_order_id: matching_offer.order_id.clone(),
                    bid: bid.clone(),
                    offer: offer.clone(),
                    residual_bid,
                    residual_offer,
                    selected_energy,
                    energy_rate: bid.bid_component.energy_rate,
                });

                if bid_remaining[position] < EPS {
                    next[position] = position + 1;
                }
                if offer_energy < EPS {
                    // This offer is exhausted.
                    break;
                }
                position = next_live(&mut next, position + 1);
            }
        }
        bid_offer_pairs
    }
}
