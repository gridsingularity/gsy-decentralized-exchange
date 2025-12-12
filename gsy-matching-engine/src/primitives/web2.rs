use chrono::NaiveDateTime;
use gsy_offchain_primitives::algorithms::PayAsBid;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;

const FLOATING_POINT_TOLERANCE: f32 = 0.00001;

pub fn serialize_datetime<S>(
	datetime: &Option<NaiveDateTime>,
	serializer: S,
) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	const FORMAT: &'static str = "%Y-%m-%dT%H:%M";
	match datetime {
		Some(datetime) => {
			let s = format!("{}", datetime.format(FORMAT));
			serializer.serialize_str(&s)
		},
		None => serializer.serialize_none(),
	}
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EnergyType {
	Clean,
	Battery,
	FossilFuel,
	Import,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Requirements {
	pub trading_partner_id: Option<String>,
	pub energy_type: Option<EnergyType>,
	pub preferred_energy_rate: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Attributes {
	pub energy_type: Option<EnergyType>,
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Bid {
	pub r#type: String,
	pub id: String,
	pub market_id: String,
	pub energy: f32,
	pub energy_rate: f32,
	pub original_price: f32,
	pub requirements: Option<Requirements>,
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
	pub market_id: String,
	pub energy: f32,
	pub energy_rate: f32,
	pub original_price: f32,
	pub attributes: Option<Attributes>,
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
	pub market_id: String,
}

impl PayAsBid for MatchingData {
	type Output = BidOfferMatch;

	fn pay_as_bid(&mut self) -> Vec<Self::Output> {
		let mut matches = Vec::new();
		// Removed unnecessary `mut`
		let available_bids = self.bids.clone();
		let available_offers = self.offers.clone();

		let mut preference_matches = Vec::new();
		let mut remaining_bids_after_pref = Vec::new();
		let mut bid_matched_amounts: HashMap<String, f32> = HashMap::new();
		let mut offer_matched_amounts: HashMap<String, f32> = HashMap::new();

		let (preference_bids, non_preference_bids): (Vec<_>, Vec<_>) =
			available_bids.into_iter().partition(|b| {
				b.requirements.as_ref().and_then(|r| r.trading_partner_id.as_ref()).is_some()
			});

		for bid in preference_bids {
			let req = bid.requirements.as_ref().unwrap();
			let partner_id = req.trading_partner_id.as_ref().unwrap();
			// Removed unused `matched_in_phase1` variable

			if let Some(offer_index) = available_offers.iter().position(|o| &o.seller == partner_id)
			{
				let offer = &available_offers[offer_index];

				let bid_energy_needed =
					bid.energy - *bid_matched_amounts.get(&bid.id).unwrap_or(&0.0);
				let offer_energy_available =
					offer.energy - *offer_matched_amounts.get(&offer.id).unwrap_or(&0.0);
				let selected_energy = bid_energy_needed.min(offer_energy_available);

				if selected_energy > FLOATING_POINT_TOLERANCE {
					let trade_rate = req.preferred_energy_rate.unwrap_or(bid.energy_rate);

					preference_matches.push(BidOfferMatch {
						market_id: offer.market_id.clone(),
						time_slot: offer.time_slot,
						bid: bid.clone(),
						selected_energy,
						offer: offer.clone(),
						trade_rate,
					});

					*bid_matched_amounts.entry(bid.id.clone()).or_insert(0.0) += selected_energy;
					*offer_matched_amounts.entry(offer.id.clone()).or_insert(0.0) +=
						selected_energy;
				}
			}

			let matched_amount = *bid_matched_amounts.get(&bid.id).unwrap_or(&0.0);
			if bid.energy - matched_amount > FLOATING_POINT_TOLERANCE {
				let mut residual_bid = bid.clone();
				residual_bid.energy -= matched_amount;
				remaining_bids_after_pref.push(residual_bid);
			}
		}

		remaining_bids_after_pref.extend(non_preference_bids);

		let mut remaining_offers_after_pref = Vec::new();
		for offer in available_offers {
			let matched_amount = *offer_matched_amounts.get(&offer.id).unwrap_or(&0.0);
			if offer.energy - matched_amount > FLOATING_POINT_TOLERANCE {
				let mut residual_offer = offer.clone();
				residual_offer.energy -= matched_amount;
				remaining_offers_after_pref.push(residual_offer);
			}
		}

		matches.extend(preference_matches);

		remaining_bids_after_pref
			.sort_by(|a, b| b.energy_rate.partial_cmp(&a.energy_rate).unwrap());
		remaining_offers_after_pref
			.sort_by(|a, b| a.energy_rate.partial_cmp(&b.energy_rate).unwrap());

		let mut available_order_energy: HashMap<String, f32> = HashMap::new();

		for offer in remaining_offers_after_pref.iter() {
			for bid in remaining_bids_after_pref.iter() {
				if offer.seller == bid.buyer
					|| (offer.energy_rate - bid.energy_rate) > FLOATING_POINT_TOLERANCE
				{
					continue;
				}

				if !available_order_energy.contains_key(bid.id.as_str()) {
					available_order_energy.insert(bid.id.clone(), bid.energy);
				}
				if !available_order_energy.contains_key(offer.id.as_str()) {
					available_order_energy.insert(offer.id.clone(), offer.energy);
				}

				let offer_energy = *available_order_energy.get(&offer.id).unwrap();
				let bid_energy = *available_order_energy.get(&bid.id).unwrap();

				let selected_energy = offer_energy.min(bid_energy);

				if selected_energy <= FLOATING_POINT_TOLERANCE {
					continue;
				}

				*available_order_energy.get_mut(&bid.id).unwrap() -= selected_energy;
				*available_order_energy.get_mut(&offer.id).unwrap() -= selected_energy;

				matches.push(BidOfferMatch {
					market_id: offer.market_id.clone(),
					time_slot: offer.time_slot,
					bid: bid.clone(),
					selected_energy,
					trade_rate: bid.energy_rate,
					offer: offer.clone(),
				});

				if let Some(offer_residual_energy) = available_order_energy.get(offer.id.as_str()) {
					if *offer_residual_energy <= FLOATING_POINT_TOLERANCE {
						break;
					}
				}
			}
		}
		matches
	}
}
