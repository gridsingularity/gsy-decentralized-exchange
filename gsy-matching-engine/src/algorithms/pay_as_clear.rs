use crate::primitives::web3::{BidOfferMatch as Web3BidOfferMatch};
use crate::primitives::web2::{Bid as Web2Bid, Offer as Web2Offer, BidOfferMatch as Web2BidOfferMatch};
use gsy_offchain_primitives::service_to_node_schema::orders::{OrderComponent as Web3OrderComponent, Bid as Web3BidPrimitive, Offer as Web3OfferPrimitive};
use std::collections::{BTreeMap, HashMap};
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};


// --- Common utility ---
#[derive(Debug, Clone, PartialEq)]
pub struct ClearingInfo<T> {
    pub rate: T,
    pub energy: T,
}

// For f32 comparisons (Web2)
const MATCH_FLOATING_POINT_TOLERANCE_F32: f32 = 1e-8;


// --- PayAsClear Trait Definition ---
pub trait PayAsClear {
    type Output;
    type BidType;
    type OfferType;

    fn pay_as_clear(&mut self) -> Vec<Self::Output>;
}


// --- Helper functions for Web3 (u64 based) ---

type Web3RateEnergyMap = BTreeMap<u64, u64>;

fn get_web3_bid_id(bid: &Web3BidPrimitive) -> H256 {
    BlakeTwo256::hash_of(&(bid.buyer.clone(), bid.nonce.clone()))
}

fn get_web3_offer_id(offer: &Web3OfferPrimitive) -> H256 {
    BlakeTwo256::hash_of(&(offer.seller.clone(), offer.nonce.clone()))
}


fn accumulated_energy_per_rate_web3_bids(sorted_bids: &[Web3BidPrimitive]) -> Web3RateEnergyMap {
    // sorted_bids are high price to low price
    let mut cumulative_bids_desc_rate: Web3RateEnergyMap = BTreeMap::new();
    let mut current_energy: u64 = 0;
    for bid in sorted_bids.iter() {
        current_energy = current_energy.saturating_add(bid.bid_component.energy);
        cumulative_bids_desc_rate.insert(bid.bid_component.energy_rate, current_energy);
    }

    // Convert to ascending rate keys, values are demand at this rate or higher
    let mut bids_rate_energy_asc: Web3RateEnergyMap = BTreeMap::new();
    for (rate, energy) in cumulative_bids_desc_rate.iter().rev() {
        bids_rate_energy_asc.insert(*rate, *energy);
    }
    bids_rate_energy_asc
}

fn accumulated_energy_per_rate_web3_offers(sorted_offers: &[Web3OfferPrimitive]) -> Web3RateEnergyMap {
    // sorted_offers are low price to high price
    let mut offers_rate_energy_asc: Web3RateEnergyMap = BTreeMap::new();
    let mut current_energy: u64 = 0;
    for offer in sorted_offers.iter() {
        current_energy = current_energy.saturating_add(offer.offer_component.energy);
        offers_rate_energy_asc.insert(offer.offer_component.energy_rate, current_energy);
    }
    offers_rate_energy_asc
}

fn clearing_point_from_supply_demand_curve_web3(
    bids_rate_energy: &Web3RateEnergyMap, // Ascending rate -> cumulative demand at or above this rate
    offers_rate_energy: &Web3RateEnergyMap, // Ascending rate -> cumulative supply at or below this rate
) -> Option<ClearingInfo<u64>> {
    let mut candidates1: Vec<ClearingInfo<u64>> = Vec::new();
    for (b_rate, b_energy) in bids_rate_energy.iter() {
        for (o_rate, o_energy) in offers_rate_energy.iter() {
            if o_rate <= b_rate {
                if o_energy >= b_energy {
                    candidates1.push(ClearingInfo { rate: *b_rate, energy: *b_energy });
                }
            }
        }
    }

    if !candidates1.is_empty() {
        // Python returns clearing[0] from a list constructed by iterating b_rate then o_rate.
        // This corresponds to the first candidate found with the lowest b_rate,
        // and for that b_rate, the lowest o_rate.
        // Our BTreeMaps are iterated in key-sorted order, so candidates1 is already sorted this way.
        return Some(candidates1[0].clone());
    }

    let mut candidates2: Vec<ClearingInfo<u64>> = Vec::new();
    for (b_rate, b_energy) in bids_rate_energy.iter() {
        for (o_rate, o_energy) in offers_rate_energy.iter() {
            if o_rate <= b_rate {
                if o_energy < b_energy {
                    candidates2.push(ClearingInfo { rate: *b_rate, energy: *o_energy });
                }
            }
        }
    }

    if !candidates2.is_empty() {
        // Python returns clearing[-1]. This is the last candidate, corresponding to
        // highest b_rate, and for that b_rate, highest o_rate.
        return candidates2.last().cloned();
    }

    None
}

pub fn get_clearing_point_web3(
    sorted_bids: &[Web3BidPrimitive],
    sorted_offers: &[Web3OfferPrimitive],
) -> Option<ClearingInfo<u64>> {
    if sorted_bids.is_empty() || sorted_offers.is_empty() {
        return None;
    }

    let cumulative_bids_asc = accumulated_energy_per_rate_web3_bids(sorted_bids);
    let cumulative_offers_asc = accumulated_energy_per_rate_web3_offers(sorted_offers);

    clearing_point_from_supply_demand_curve_web3(&cumulative_bids_asc, &cumulative_offers_asc)
}


pub fn create_bid_offer_matches_web3(
    sorted_bids: &[Web3BidPrimitive],
    sorted_offers: &[Web3OfferPrimitive],
    clearing_info: &ClearingInfo<u64>,
    market_id: u8,
    time_slot: u64,
) -> Vec<Web3BidOfferMatch> {
    let clearing_rate = clearing_info.rate;
    let mut clearing_energy_remaining = clearing_info.energy;
    let mut bid_offer_matches = Vec::new();

    if clearing_energy_remaining == 0 {
        return bid_offer_matches;
    }

    let mut residual_offer_energies: HashMap<H256, u64> = HashMap::new();
    let mut offer_idx = 0;

    for bid_original in sorted_bids.iter() {
        if bid_original.bid_component.energy_rate < clearing_rate { // Bid price too low
            continue;
        }
        if clearing_energy_remaining == 0 {
            break;
        }

        let _bid_id = get_web3_bid_id(bid_original);
        let mut bid_energy_to_match = bid_original.bid_component.energy;

        while bid_energy_to_match > 0 && offer_idx < sorted_offers.len() && clearing_energy_remaining > 0 {
            let offer_original = &sorted_offers[offer_idx];
            if offer_original.offer_component.energy_rate > clearing_rate { // Offer price too high
                 if offer_original.offer_component.energy_rate > clearing_rate {
                    // This offer is not eligible at this clearing price.
                    break;
                }

            }

            let offer_id = get_web3_offer_id(offer_original);
            let current_offer_energy_available = residual_offer_energies
                .get(&offer_id)
                .cloned()
                .unwrap_or(offer_original.offer_component.energy);

            if current_offer_energy_available == 0 {
                offer_idx += 1;
                continue;
            }

            let energy_for_this_pairing = bid_energy_to_match
                .min(current_offer_energy_available)
                .min(clearing_energy_remaining);

            if energy_for_this_pairing == 0 {
                // This can happen if clearing_energy_remaining became 0
                if clearing_energy_remaining == 0 { break; }
                 offer_idx += 1; 
                 continue;
            }

            // Create residual Bid
            let remaining_bid_energy_after_match = bid_energy_to_match.saturating_sub(energy_for_this_pairing);
            let residual_bid_struct = if remaining_bid_energy_after_match > 0 {
                Some(Web3BidPrimitive {
                    buyer: bid_original.buyer.clone(),
                    nonce: bid_original.nonce.saturating_add(1), 
                    bid_component: Web3OrderComponent {
                        energy: remaining_bid_energy_after_match,
                        ..bid_original.bid_component.clone()
                    },
                })
            } else {
                None
            };

            // Create residual Offer
            let remaining_offer_energy_after_match = current_offer_energy_available.saturating_sub(energy_for_this_pairing);
            let residual_offer_struct = if remaining_offer_energy_after_match > 0 {
                Some(Web3OfferPrimitive {
                    seller: offer_original.seller.clone(),
                    nonce: offer_original.nonce.saturating_add(1), 
                    offer_component: Web3OrderComponent {
                        energy: remaining_offer_energy_after_match,
                        ..offer_original.offer_component.clone()
                    },
                })
            } else {
                None
            };
            
            let match_item = Web3BidOfferMatch {
                market_id,
                time_slot, 
                bid: bid_original.clone(),
                offer: offer_original.clone(), 
                selected_energy: energy_for_this_pairing,
                energy_rate: clearing_rate,
                residual_bid: residual_bid_struct,
                residual_offer: residual_offer_struct,
            };
            bid_offer_matches.push(match_item);

            bid_energy_to_match = remaining_bid_energy_after_match;
            residual_offer_energies.insert(offer_id, remaining_offer_energy_after_match);
            clearing_energy_remaining = clearing_energy_remaining.saturating_sub(energy_for_this_pairing);

            if remaining_offer_energy_after_match == 0 {
                offer_idx += 1;
            }
        }
    }
    bid_offer_matches
}

// --- Helper functions for Web2 (f32 based) ---

type Web2RateEnergyVec = Vec<(f32, f32)>;

fn accumulated_energy_per_rate_web2_bids(sorted_bids: &[Web2Bid]) -> Web2RateEnergyVec {
    // sorted_bids are high price to low price
    if sorted_bids.is_empty() { return Vec::new(); }

    let mut energy_per_rate: BTreeMap<u32, f32> = BTreeMap::new(); // rate.to_bits() -> sum_energy_at_this_rate
    for bid in sorted_bids.iter() {
        *energy_per_rate.entry(bid.energy_rate.to_bits()).or_default() += bid.energy;
    }

    // Create cumulative list (descending rates, as bids are sorted high-to-low)
    let mut cumulative_bids_desc_rate: Vec<(f32, f32)> = Vec::new();
    let mut cumulative_sum = 0.0;
    // Iterate distinct rates from sorted_bids to maintain descending order
    let mut distinct_rates_desc: Vec<f32> = sorted_bids.iter().map(|b| b.energy_rate).collect();
    distinct_rates_desc.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)); // Desc
    distinct_rates_desc.dedup();

    for rate_float in distinct_rates_desc {
        cumulative_sum += energy_per_rate.get(&rate_float.to_bits()).unwrap_or(&0.0);
        cumulative_bids_desc_rate.push((rate_float, cumulative_sum));
    }
    
    // Convert to ascending rate keys for the curve matching logic
    let mut bids_rate_energy_asc = cumulative_bids_desc_rate;
    bids_rate_energy_asc.reverse(); // Rates are ascending, energy is "demand at or above this rate"
    bids_rate_energy_asc
}

fn accumulated_energy_per_rate_web2_offers(sorted_offers: &[Web2Offer]) -> Web2RateEnergyVec {
    // sorted_offers are low price to high price
    if sorted_offers.is_empty() { return Vec::new(); }

    let mut energy_per_rate: BTreeMap<u32, f32> = BTreeMap::new(); // rate.to_bits() -> sum_energy_at_this_rate
    for offer in sorted_offers.iter() {
        *energy_per_rate.entry(offer.energy_rate.to_bits()).or_default() += offer.energy;
    }

    // Create cumulative list (ascending rates, as offers are sorted low-to-high)
    let mut offers_rate_energy_asc: Vec<(f32,f32)> = Vec::new();
    let mut cumulative_sum = 0.0;
    // Iterate distinct rates from sorted_offers to maintain ascending order
    let mut distinct_rates_asc: Vec<f32> = sorted_offers.iter().map(|o| o.energy_rate).collect();
    distinct_rates_asc.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)); 
    distinct_rates_asc.dedup();
    
    for rate_float in distinct_rates_asc {
        cumulative_sum += energy_per_rate.get(&rate_float.to_bits()).unwrap_or(&0.0);
        offers_rate_energy_asc.push((rate_float, cumulative_sum)); // Energy is "supply at or below this rate"
    }
    offers_rate_energy_asc
}

fn clearing_point_from_supply_demand_curve_web2(
    bids_rate_energy: &Web2RateEnergyVec, // Ascending rate -> cumulative demand at or above this rate
    offers_rate_energy: &Web2RateEnergyVec, // Ascending rate -> cumulative supply at or below this rate
) -> Option<ClearingInfo<f32>> {
    let mut candidates1: Vec<ClearingInfo<f32>> = Vec::new();
    for (b_rate, b_energy) in bids_rate_energy.iter() {
        for (o_rate, o_energy) in offers_rate_energy.iter() {
            if o_rate <= &(b_rate + MATCH_FLOATING_POINT_TOLERANCE_F32) { // o_rate <= b_rate
                if o_energy >= &(b_energy - MATCH_FLOATING_POINT_TOLERANCE_F32) { // o_energy >= b_energy
                    candidates1.push(ClearingInfo { rate: *b_rate, energy: *b_energy });
                }
            }
        }
    }

    if !candidates1.is_empty() {
        return Some(candidates1[0].clone()); // First one (lowest b_rate, then lowest o_rate)
    }

    let mut candidates2: Vec<ClearingInfo<f32>> = Vec::new();
    for (b_rate, b_energy) in bids_rate_energy.iter() {
        for (o_rate, o_energy) in offers_rate_energy.iter() {
            if o_rate <= &(b_rate + MATCH_FLOATING_POINT_TOLERANCE_F32) { // o_rate <= b_rate
                 // o_energy < b_energy
                if o_energy < &(b_energy - MATCH_FLOATING_POINT_TOLERANCE_F32) {
                    candidates2.push(ClearingInfo { rate: *b_rate, energy: *o_energy });
                }
            }
        }
    }

    if !candidates2.is_empty() {
        return candidates2.last().cloned(); // Last one (highest b_rate, then highest o_rate)
    }
    None
}


pub fn get_clearing_point_web2(
    sorted_bids: &[Web2Bid],
    sorted_offers: &[Web2Offer],
) -> Option<ClearingInfo<f32>> {
    if sorted_bids.is_empty() || sorted_offers.is_empty() {
        return None;
    }

    let cumulative_bids_asc = accumulated_energy_per_rate_web2_bids(sorted_bids);
    let cumulative_offers_asc = accumulated_energy_per_rate_web2_offers(sorted_offers);
    
    clearing_point_from_supply_demand_curve_web2(&cumulative_bids_asc, &cumulative_offers_asc)
}


pub fn create_bid_offer_matches_web2(
    sorted_bids: &[Web2Bid],
    sorted_offers: &[Web2Offer],
    clearing_info: &ClearingInfo<f32>,
    market_id: String,
) -> Vec<Web2BidOfferMatch> {
    let clearing_rate = clearing_info.rate;
    let mut clearing_energy_remaining = clearing_info.energy;
    let mut bid_offer_matches = Vec::new();

    if clearing_energy_remaining <= MATCH_FLOATING_POINT_TOLERANCE_F32 {
        return bid_offer_matches;
    }

    let mut residual_offer_energies: HashMap<String, f32> = HashMap::new();
    let mut offer_idx = 0;

    for bid_original in sorted_bids.iter() {
        // Bid must be willing to pay at least the clearing rate
        if bid_original.energy_rate < clearing_rate - MATCH_FLOATING_POINT_TOLERANCE_F32 {
            continue;
        }
        if clearing_energy_remaining <= MATCH_FLOATING_POINT_TOLERANCE_F32 {
            break;
        }

        let mut bid_energy_to_match = bid_original.energy;

        while bid_energy_to_match > MATCH_FLOATING_POINT_TOLERANCE_F32 
              && offer_idx < sorted_offers.len() 
              && clearing_energy_remaining > MATCH_FLOATING_POINT_TOLERANCE_F32 {
            
            let offer_original = &sorted_offers[offer_idx];

            // Offer must be willing to sell at most the clearing rate
            if offer_original.energy_rate > clearing_rate + MATCH_FLOATING_POINT_TOLERANCE_F32 {
                break; 
            }

            let offer_id = offer_original.id.clone();
            let current_offer_energy_available = residual_offer_energies
                .get(&offer_id)
                .cloned()
                .unwrap_or(offer_original.energy);

            if current_offer_energy_available <= MATCH_FLOATING_POINT_TOLERANCE_F32 {
                offer_idx += 1;
                continue;
            }
            
            let mut energy_for_this_pairing = bid_energy_to_match.min(current_offer_energy_available);
            energy_for_this_pairing = energy_for_this_pairing.min(clearing_energy_remaining);


            if energy_for_this_pairing <= MATCH_FLOATING_POINT_TOLERANCE_F32 {
                 if clearing_energy_remaining <= MATCH_FLOATING_POINT_TOLERANCE_F32 { break; }
                 offer_idx += 1; 
                 continue;
            }
            
            let match_item = Web2BidOfferMatch {
                market_id: market_id.clone(),
                time_slot: bid_original.time_slot, 
                bid: bid_original.clone(),
                selected_energy: energy_for_this_pairing,
                offer: offer_original.clone(),
                trade_rate: clearing_rate,
            };
            bid_offer_matches.push(match_item);

            bid_energy_to_match -= energy_for_this_pairing;
            residual_offer_energies.insert(offer_id, current_offer_energy_available - energy_for_this_pairing);
            clearing_energy_remaining -= energy_for_this_pairing;

            if (current_offer_energy_available - energy_for_this_pairing) <= MATCH_FLOATING_POINT_TOLERANCE_F32 {
                offer_idx += 1;
            }
        }
    }
    bid_offer_matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::web3::{MatchingData as Web3MatchingData};
    use crate::primitives::web2::{Bid as Web2ApiBid, Offer as Web2ApiOffer, MatchingData as Web2MatchingData};
    use gsy_offchain_primitives::service_to_node_schema::orders::{
        Bid as Web3BidPrimitive, Offer as Web3OfferPrimitive, OrderComponent as Web3OrderComponent,
    };
    use subxt::utils::AccountId32;
    use subxt::ext::sp_core::H256;
    use chrono::NaiveDateTime;

    const TEST_MATCH_FLOATING_POINT_TOLERANCE_F32: f32 = 1e-5; 

    // --- Mock Data Helpers ---
    fn mock_web3_bid(buyer: String, nonce: u32, energy: u64, rate: u64, time_slot: u64) -> Web3BidPrimitive {
        Web3BidPrimitive {
            buyer,
            nonce,
            bid_component: Web3OrderComponent {
                area_uuid: H256::from_low_u64_be(1),
                market_id: H256::from_low_u64_be(1), 
                time_slot,
                creation_time: 1000,
                energy,
                energy_rate: rate,
            },
        }
    }

    fn mock_web3_offer(seller: String, nonce: u32, energy: u64, rate: u64, time_slot: u64) -> Web3OfferPrimitive {
        Web3OfferPrimitive {
            seller,
            nonce,
            offer_component: Web3OrderComponent {
                area_uuid: H256::from_low_u64_be(2), 
                market_id: H256::from_low_u64_be(1), 
                time_slot,
                creation_time: 1000,
                energy, 
                energy_rate: rate,
            },
        }
    }
    
    fn mock_account_id(id: u8) -> String {
        AccountId32::from([id; 32]).to_string()
    }

    fn mock_web2_bid(id: &str, energy: f32, rate: f32, time_slot_str: &str) -> Web2ApiBid {
        Web2ApiBid {
            r#type: "bid".to_string(),
            id: id.to_string(),
            energy,
            energy_rate: rate,
            original_price: rate,
            requirements: None,
            buyer_origin: "origin_b".to_string(),
            buyer_origin_id: "origin_id_b".to_string(),
            buyer_id: format!("buyer_{}", id),
            buyer: format!("buyer_name_{}", id),
            time_slot: NaiveDateTime::parse_from_str(time_slot_str, "%Y-%m-%dT%H:%M:%S").ok(),
            creation_time: NaiveDateTime::parse_from_str("2024-01-01T10:00:00", "%Y-%m-%dT%H:%M:%S").ok(),
        }
    }

    fn mock_web2_offer(id: &str, energy: f32, rate: f32, time_slot_str: &str) -> Web2ApiOffer {
        Web2ApiOffer {
            r#type: "offer".to_string(),
            id: id.to_string(),
            energy,
            energy_rate: rate,
            original_price: rate,
            requirements: None,
            seller_origin: "origin_s".to_string(),
            seller_origin_id: "origin_id_s".to_string(),
            seller_id: format!("seller_{}", id),
            seller: format!("seller_name_{}", id),
            time_slot: NaiveDateTime::parse_from_str(time_slot_str, "%Y-%m-%dT%H:%M:%S").ok(),
            creation_time: NaiveDateTime::parse_from_str("2024-01-01T10:00:00", "%Y-%m-%dT%H:%M:%S").ok(),
        }
    }

    // --- Web3 Tests ---
    #[test]
    fn test_accumulated_energy_per_rate_web3_bids_empty() {
        let bids = Vec::<Web3BidPrimitive>::new();
        let acc_bids = accumulated_energy_per_rate_web3_bids(&bids);
        assert!(acc_bids.is_empty());
    }

    #[test]
    fn test_accumulated_energy_per_rate_web3_bids_simple() {
        // Bids sorted by rate: (10, 100), (20, 90) -> (Energy, Rate)
        // Should be sorted high rate to low rate for input to function
        let _bids = vec![
            mock_web3_bid(mock_account_id(1), 1, 20, 90, 1), // E=20, R=90
            mock_web3_bid(mock_account_id(2), 1, 10, 100, 1), // E=10, R=100
        ];
        
        let sorted_bids_for_test = vec![
             mock_web3_bid(mock_account_id(2), 1, 10, 100, 1), // E=10, R=100
             mock_web3_bid(mock_account_id(1), 1, 20, 90, 1), // E=20, R=90
        ];


        let acc_bids = accumulated_energy_per_rate_web3_bids(&sorted_bids_for_test);
        
        let mut expected = Web3RateEnergyMap::new();
        expected.insert(90, 30);
        expected.insert(100, 10);
        assert_eq!(acc_bids, expected);
    }

    #[test]
    fn test_accumulated_energy_per_rate_web3_offers_simple() {
        // Offers sorted by rate: (10, 80), (15, 85) -> (Energy, Rate)
        let sorted_offers_for_test = vec![
            mock_web3_offer(mock_account_id(3), 1, 10, 80, 1), // E=10, R=80
            mock_web3_offer(mock_account_id(4), 1, 15, 85, 1), // E=15, R=85
        ];
        let acc_offers = accumulated_energy_per_rate_web3_offers(&sorted_offers_for_test);

        let mut expected = Web3RateEnergyMap::new();
        expected.insert(80, 10);
        expected.insert(85, 25);
        assert_eq!(acc_offers, expected);
    }


    #[test]
    fn test_clearing_point_from_supply_demand_curve_web3_case1() {
        // bids_rate_energy (demand): (90 -> 30), (100 -> 10)
        // offers_rate_energy (supply): (80 -> 10), (85 -> 25)
        let mut bids_demand = Web3RateEnergyMap::new();
        bids_demand.insert(90, 30); // At rate 90 or more, demand is 30
        bids_demand.insert(100, 10); // At rate 100 or more, demand is 10

        let mut offers_supply = Web3RateEnergyMap::new();
        offers_supply.insert(80, 10); // At rate 80 or less, supply is 10
        offers_supply.insert(85, 25); // At rate 85 or less, supply is 25

        let mut bids_py_example = Web3RateEnergyMap::new();
        bids_py_example.insert(10,30); bids_py_example.insert(20,20); bids_py_example.insert(30,10);
        
        let mut offers_py_example = Web3RateEnergyMap::new();
        offers_py_example.insert(10,5); offers_py_example.insert(20,15); offers_py_example.insert(30,25);

        let clearing = clearing_point_from_supply_demand_curve_web3(&bids_py_example, &offers_py_example).unwrap();
        assert_eq!(clearing.rate, 30);
        assert_eq!(clearing.energy, 10);
    }

    #[test]
    fn test_get_clearing_point_web3_no_match() {
        let bids = vec![mock_web3_bid(mock_account_id(1), 1, 10, 10, 1)];
        let offers = vec![mock_web3_offer(mock_account_id(2), 1, 10, 20, 1)]; 
        let clearing = get_clearing_point_web3(&bids, &offers);
        assert!(clearing.is_none());
    }

    #[test]
    fn test_pay_as_clear_web3_full_match() {
        let bids = vec![
            mock_web3_bid(mock_account_id(1), 1, 10, 100, 1), 
        ];
        let offers = vec![
            mock_web3_offer(mock_account_id(2), 1, 10, 90, 1),  
        ];
        let mut data = Web3MatchingData { bids, offers, market_id: 1 };
        let matches = data.pay_as_clear();

        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        
        assert_eq!(m.energy_rate, 100);
        assert_eq!(m.selected_energy, 10);
        assert_eq!(m.bid.bid_component.energy_rate, 100);
        assert_eq!(m.offer.offer_component.energy_rate, 90);
    }
    
    #[test]
    fn test_pay_as_clear_web3_example() {
        
        let bids = vec![
            mock_web3_bid(mock_account_id(1), 1, 20, 10, 1), 
            mock_web3_bid(mock_account_id(2), 1, 30, 8, 1), 
        ];
        let offers = vec![
            mock_web3_offer(mock_account_id(3), 1, 15, 5, 1),  
            mock_web3_offer(mock_account_id(4), 1, 25, 7, 1),  
        ];
        let mut data = Web3MatchingData { bids, offers, market_id: 1 };
        let matches = data.pay_as_clear();
        
        assert_eq!(matches.len(), 2);
        let total_cleared_energy: u64 = matches.iter().map(|m| m.selected_energy).sum();
        assert_eq!(total_cleared_energy, 20);
        for m in matches.iter() {
            assert_eq!(m.energy_rate, 10);
        }
        assert_eq!(matches[0].bid.bid_component.energy_rate, 10); 
        assert_eq!(matches[0].offer.offer_component.energy_rate, 5); 
        assert_eq!(matches[0].selected_energy, 15);
        
        assert_eq!(matches[1].bid.bid_component.energy_rate, 10);
        assert_eq!(matches[1].offer.offer_component.energy_rate, 7); 
        assert_eq!(matches[1].selected_energy, 5);
    }

    // --- Web2 Tests ---
    #[test]
    fn test_accumulated_energy_per_rate_web2_bids_simple() {
        let sorted_bids_for_test = vec![
             mock_web2_bid("b2", 10.0, 100.0, "2024-01-01T12:00:00"),
             mock_web2_bid("b1", 20.0, 90.0, "2024-01-01T12:00:00"),
        ];
        let acc_bids = accumulated_energy_per_rate_web2_bids(&sorted_bids_for_test);
        
        let expected = vec![(90.0, 30.0), (100.0, 10.0)];
        assert_eq!(acc_bids.len(), expected.len());
        for (i, (r, e)) in acc_bids.iter().enumerate() {
            assert!((r - expected[i].0).abs() < TEST_MATCH_FLOATING_POINT_TOLERANCE_F32);
            assert!((e - expected[i].1).abs() < TEST_MATCH_FLOATING_POINT_TOLERANCE_F32);
        }
    }
    
    #[test]
    fn test_pay_as_clear_web2_full_match() {
        let bids = vec![
            mock_web2_bid("b1", 10.0, 100.0, "2024-01-01T12:00:00"),
        ];
        let offers = vec![
            mock_web2_offer("s1", 10.0, 90.0, "2024-01-01T12:00:00"),
        ];
        let mut data = Web2MatchingData { bids, offers, market_id: "M1".to_string() };
        let matches = data.pay_as_clear();

        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        
        assert!((m.bid.energy_rate - 100.0).abs() < TEST_MATCH_FLOATING_POINT_TOLERANCE_F32);
        assert!((m.selected_energy - 10.0).abs() < TEST_MATCH_FLOATING_POINT_TOLERANCE_F32);
    }

    #[test]
    fn test_pay_as_clear_web2_no_bids() {
        let bids = Vec::new();
        let offers = vec![
            mock_web2_offer("s1", 10.0, 90.0, "2024-01-01T12:00:00"),
        ];
        let mut data = Web2MatchingData { bids, offers, market_id: "M1".to_string() };
        let matches = data.pay_as_clear();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_pay_as_clear_web2_no_offers() {
        let bids = vec![
            mock_web2_bid("b1", 10.0, 100.0, "2024-01-01T12:00:00"),
        ];
        let offers = Vec::new();
        let mut data = Web2MatchingData { bids, offers, market_id: "M1".to_string() };
        let matches = data.pay_as_clear();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_pay_as_clear_web2_no_overlap() {
        let bids = vec![
            mock_web2_bid("b1", 10.0, 80.0, "2024-01-01T12:00:00"), 
        ];
        let offers = vec![
            mock_web2_offer("s1", 10.0, 90.0, "2024-01-01T12:00:00"), 
        ];
        let mut data = Web2MatchingData { bids, offers, market_id: "M1".to_string() };
        let matches = data.pay_as_clear();
        assert!(matches.is_empty()); 
    }
}