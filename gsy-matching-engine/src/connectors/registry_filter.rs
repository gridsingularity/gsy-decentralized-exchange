//! Match only orders the chain will accept.
//!
//! `trades_settlement::settle_trades` aborts on the first match whose `clear_order` fails, and
//! `pay_as_bid` is deterministic, so a single order that is `Open` in storage but not `Open` in
//! `OrderbookRegistry.OrdersRegistry` would fail its market's proposal on every cycle. The
//! matching engine therefore looks every order up in the registry before matching, and keeps only
//! those whose entry exists and is `Open`. Filtering happens before matching, not only before
//! settlement: a rejected order dropped at settlement would still take its counterpart's single
//! match slot every cycle.
//!
//! The filter itself is a pure function over a lookup ([`filter_registered_open`]), so it can be
//! tested without a node; the connector supplies a lookup backed by subxt.

use gsy_offchain_primitives::types::{Bid, BidOfferMatch, Offer, Order};
use std::collections::HashSet;
use subxt::config::{substrate::BlakeTwo256, Hasher};
use subxt::utils::{AccountId32, H256};

/// The status of an order in `OrderbookRegistry.OrdersRegistry`, reduced to what the filter needs.
/// A missing entry is not a status: lookups return `None` for it (the map is a `ValueQuery` whose
/// default is `Open`, so reading a missing key with a default would wrongly report `Open`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryStatus {
	Open,
	Executed,
	Deleted,
}

/// An `Open` order read from the off-chain storage, in node units, with its storage `_id`.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredOrder {
	/// The storage `_id`: the order hash as the orderbook worker relayed it.
	pub id: String,
	pub order: Order,
}

impl StoredOrder {
	/// The account the registry keys the order by: the buyer of a bid, the seller of an offer.
	pub fn owner(&self) -> &AccountId32 {
		match &self.order {
			Order::Bid(bid) => &bid.buyer,
			Order::Offer(offer) => &offer.seller,
		}
	}

	/// The order hash the chain checks in `clear_order`: BlakeTwo256 of the SCALE-encoded node
	/// order. Equal to the storage `_id` as long as the storage round trip is exact.
	pub fn hash(&self) -> H256 {
		match &self.order {
			Order::Bid(bid) => bid_hash(bid),
			Order::Offer(offer) => offer_hash(offer),
		}
	}
}

/// The node's hash of a bid (BlakeTwo256 of its SCALE encoding, as in `clear_order`).
pub fn bid_hash(bid: &Bid) -> H256 {
	BlakeTwo256.hash_of(bid)
}

/// The node's hash of an offer (BlakeTwo256 of its SCALE encoding, as in `clear_order`).
pub fn offer_hash(offer: &Offer) -> H256 {
	BlakeTwo256.hash_of(offer)
}

/// `(bid hash, offer hash)` of every match of a proposal, for logging a failed settlement.
pub fn match_order_hashes(matches: &[BidOfferMatch]) -> Vec<(H256, H256)> {
	matches
		.iter()
		.map(|bid_offer_match| (bid_hash(&bid_offer_match.bid), offer_hash(&bid_offer_match.offer)))
		.collect()
}

/// An order the filter dropped, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct DroppedOrder {
	/// The hash the registry was queried with (see [`StoredOrder::hash`]).
	pub hash: H256,
	/// The storage `_id`, logged too in case it differs from `hash`.
	pub id: String,
	pub owner: AccountId32,
	/// The registry status; `None` when the registry has no entry for the order.
	pub status: Option<RegistryStatus>,
}

/// The result of [`filter_registered_open`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilteredOrders {
	pub bids: Vec<Bid>,
	pub offers: Vec<Offer>,
	pub dropped: Vec<DroppedOrder>,
}

/// Keep the orders whose registry entry exists and is `Open`, split into bids and offers in their
/// original order, and report the rest as dropped. `lookup(owner, hash)` returns the registry
/// status of `OrderReference { user_id: owner, hash }`, or `None` when there is no entry; a
/// missing entry is dropped, never treated as `Open`.
pub fn filter_registered_open(
	orders: Vec<StoredOrder>,
	mut lookup: impl FnMut(&AccountId32, &H256) -> Option<RegistryStatus>,
) -> FilteredOrders {
	let mut filtered = FilteredOrders::default();
	for stored in orders {
		let hash = stored.hash();
		match lookup(stored.owner(), &hash) {
			Some(RegistryStatus::Open) => match stored.order {
				Order::Bid(bid) => filtered.bids.push(bid),
				Order::Offer(offer) => filtered.offers.push(offer),
			},
			status => filtered.dropped.push(DroppedOrder {
				hash,
				owner: stored.owner().clone(),
				id: stored.id,
				status,
			}),
		}
	}
	filtered
}

/// Remembers which dropped orders were already logged, so each is logged once rather than every
/// cycle. Bounded: after each cycle it holds only the hashes dropped in that cycle, so an order
/// that leaves storage's `Open` set (executed, expired) is forgotten.
#[derive(Debug, Default)]
pub struct DroppedOrderLog {
	seen: HashSet<H256>,
}

impl DroppedOrderLog {
	pub fn new() -> Self {
		Self::default()
	}

	/// Record this cycle's dropped orders and return those not reported by an earlier cycle.
	pub fn newly_dropped<'a>(&mut self, dropped: &'a [DroppedOrder]) -> Vec<&'a DroppedOrder> {
		let current: HashSet<H256> = dropped.iter().map(|order| order.hash).collect();
		let new: Vec<&DroppedOrder> = dropped
			.iter()
			.filter(|order| !self.seen.contains(&order.hash))
			.collect();
		self.seen = current;
		new
	}

	/// Number of remembered hashes.
	pub fn len(&self) -> usize {
		self.seen.len()
	}

	pub fn is_empty(&self) -> bool {
		self.seen.is_empty()
	}
}
