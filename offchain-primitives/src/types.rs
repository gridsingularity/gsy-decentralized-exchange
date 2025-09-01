use crate::algorithms::PayAsBid;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use subxt::ext::sp_core::H256;
use subxt::ext::sp_runtime::traits::{BlakeTwo256, Hash};
use subxt::utils::AccountId32;

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
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Offer {
	pub seller: AccountId32,
	pub nonce: u32,
	pub offer_component: OrderComponent,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Order {
	Bid(Bid),
	Offer(Offer),
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct BidOfferMatch {
	pub market_id: u8,
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
	pub market_id: u8,
}

impl PayAsBid for MatchingData {
	type Output = BidOfferMatch;

	fn pay_as_bid(&mut self) -> Vec<Self::Output> {
		let mut bid_offer_pairs = Vec::new();

		let mut bids = self.bids.clone();
		let mut offers = self.offers.clone();

		bids.sort_by(|a, b| b.bid_component.energy_rate.cmp(&a.bid_component.energy_rate));
		offers.sort_by(|a, b| a.offer_component.energy_rate.cmp(&b.offer_component.energy_rate));

		let mut available_order_energy: HashMap<H256, u64> = HashMap::new();

		for offer in &mut offers {
			for bid in &mut bids {
				if offer.offer_component.area_uuid == bid.bid_component.area_uuid
					|| offer.offer_component.energy == 0
					|| bid.bid_component.energy == 0
				{
					continue;
				}

				if offer.offer_component.energy_rate > bid.bid_component.energy_rate {
					continue;
				}

				let bid_id = BlakeTwo256::hash_of(&bid);
				let offer_id = BlakeTwo256::hash_of(&offer);

				let offer_energy =
					*available_order_energy.entry(offer_id).or_insert(offer.offer_component.energy);
				let bid_energy =
					*available_order_energy.entry(bid_id).or_insert(bid.bid_component.energy);

				let selected_energy = offer_energy.min(bid_energy);

				if selected_energy == 0 {
					continue;
				}

				available_order_energy.insert(bid_id, bid_energy - selected_energy);
				available_order_energy.insert(offer_id, offer_energy - selected_energy);

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
					market_id: self.market_id,
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
