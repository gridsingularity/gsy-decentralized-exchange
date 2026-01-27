use crate::algorithms::PayAsBid;
use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::Into;
use subxt::utils::H256;
use subxt::config::{substrate::BlakeTwo256, Hasher};
use subxt::utils::AccountId32;

#[subxt::subxt(runtime_metadata_path = "./metadata.scale")]
pub mod gsy_node {}

pub use crate::types::gsy_node::runtime_types::gsy_primitives::trades::BidOfferMatch as NodeBidOfferMatch;
pub use crate::types::gsy_node::runtime_types::gsy_primitives::orders::{Bid as NodeBid, Offer as NodeOffer, OrderComponent as NodeOrderComponent};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct OrderComponent {
	pub area_uuid: H256,
	pub market_id: H256,
	pub time_slot: u64,
	pub creation_time: u64,
	pub energy: u64,
	pub energy_rate: u64,
}

impl Into<NodeOrderComponent> for OrderComponent {
	fn into(self) -> NodeOrderComponent {
		NodeOrderComponent {
			area_uuid: self.area_uuid,
			market_id: self.market_id,
			time_slot: self.time_slot,
			creation_time: self.creation_time,
			energy: self.energy,
			energy_rate: self.energy_rate,
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Bid {
	pub buyer: AccountId32,
	pub nonce: u32,
	pub bid_component: OrderComponent,
}

impl Into<NodeBid<AccountId32>> for Bid {
	fn into(self) -> NodeBid<AccountId32> {
		NodeBid {
			buyer: self.buyer,
			nonce: self.nonce,
			bid_component: self.bid_component.into(),
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq, PartialOrd)]
pub struct Offer {
	pub seller: AccountId32,
	pub nonce: u32,
	pub offer_component: OrderComponent,
}


impl Into<NodeOffer<AccountId32>> for Offer {
	fn into(self) -> NodeOffer<AccountId32> {
		NodeOffer {
			seller: self.seller,
			nonce: self.nonce,
			offer_component: self.offer_component.into(),
		}
	}
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


impl Into<NodeBidOfferMatch<AccountId32, H256>> for BidOfferMatch {
	fn into(self) -> NodeBidOfferMatch<AccountId32, H256> {
		NodeBidOfferMatch {
			bid: self.bid.into(),
			offer: self.offer.into(),
			market_id: self.market_id,
			time_slot: self.time_slot,
			residual_bid: self.residual_bid.map(|bid| bid.into()),
			residual_offer: self.residual_offer.map(|offer| offer.into()),
			selected_energy: self.selected_energy,
			energy_rate: self.energy_rate,
		}
	}
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MatchingData {
	pub bids: Vec<Bid>,
	pub offers: Vec<Offer>,
	pub market_id: H256,
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

				let bid_id = BlakeTwo256.hash_of(&bid);
				let offer_id = BlakeTwo256.hash_of(&offer);

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
					market_id: offer.offer_component.market_id,
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
