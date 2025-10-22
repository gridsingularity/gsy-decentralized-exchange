use crate::orders::{Bid, Offer};
use crate::v0::{AccountId, Hash};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
pub use sp_runtime::traits::{BlakeTwo256, Hash as HashT};

/// Trade struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct Trade<AccountId32, Hash> {
	pub seller: AccountId32,
	pub buyer: AccountId32,
	pub market_id: u8,
	pub trade_uuid: Hash,
	pub creation_time: u64,
	pub time_slot: u64,
	pub offer: Offer<AccountId32>,
	pub offer_hash: Hash,
	pub bid: Bid<AccountId32>,
	pub bid_hash: Hash,
	pub residual_bid: Option<Bid<AccountId32>>,
	pub residual_offer: Option<Offer<AccountId32>>,
	pub parameters: TradeParameters<Hash>,
}

impl Trade<AccountId, Hash> {
	/// Compute the blake2-256 hash of the Trade struct.
	pub fn hash(&self) -> Hash {
		BlakeTwo256::hash_of(self)
	}
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct TradesPenalties<AccountId, Hash> {
    pub penalized_account: AccountId,
    pub market_uuid: u32,
	pub trade_uuid: Hash,
    pub penalty_energy: u64,
}


#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct TradeParameters<Hash> {
	/// The amount of energy that is traded.
	pub selected_energy: u64,
	/// The price of the traded energy.
	pub energy_rate: u64,
	/// The trade hash.
	pub trade_uuid: Hash,
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct BidOfferMatch<AccountId> {
	/// The market ID
	pub market_id: u8,
	/// The time slot
	pub time_slot: u64,
	/// The bid
	pub bid: Bid<AccountId>,
	/// The offer
	pub offer: Offer<AccountId>,
	/// The residual bid (if any)
	pub residual_bid: Option<Bid<AccountId>>,
	/// The residual offer (if any)
	pub residual_offer: Option<Offer<AccountId>>,
	/// The amount of energy that is traded.
	pub selected_energy: u64,
	/// The price of the selected energy.
	pub energy_rate: u64,
}

/// Expose function to Validate Bids/Offers Matches.
pub trait Validator {
	type AccountId;

	/// Validate a bid/offer match.
	fn validate(bid_offer_match: &BidOfferMatch<Self::AccountId>) -> bool;
	/// Check the energy amount of the bid against the selected energy amount.
	fn validate_bid_energy_component(bid_component_energy: u64, selected_energy: u64) -> bool;
	/// Check the energy amount of the offer against the selected energy amount.
	fn validate_offer_energy_component(offer_component_energy: u64, selected_energy: u64) -> bool;
	/// Check the energy rate of the bid against energy rate of the offer.
	fn validate_energy_rate(energy_rate: u64, offer_energy_rate: u64) -> bool;
	/// Check the residual bid in the bid/offer match.
	fn validate_residual_bid(
		residual_bid: &Bid<Self::AccountId>,
		bid: &Bid<Self::AccountId>,
		selected_energy: u64,
	) -> bool;
	/// Check the residual offer in the bid/offer match.
	fn validate_residual_offer(
		residual_offer: &Offer<Self::AccountId>,
		offer: &Offer<Self::AccountId>,
		selected_energy: u64,
	) -> bool;
	/// Check the time slot of the bid/offer match.
	fn validate_time_slots(
		bid_time_slot: u64,
		offer_time_slot: u64,
		proposed_match_market_slot: u64,
	) -> bool;
}
