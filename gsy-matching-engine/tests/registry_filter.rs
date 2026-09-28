//! The registry filter: the matching engine matches only orders whose
//! `OrderbookRegistry.OrdersRegistry` entry exists and is `Open`.

use gsy_matching_engine::connectors::registry_filter::{
	bid_hash, filter_registered_open, match_order_hashes, offer_hash, DroppedOrder,
	DroppedOrderLog, RegistryStatus, StoredOrder,
};
use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::types::{Bid, MatchingData, Offer, Order, OrderComponent};
use gsy_offchain_primitives::utils::h256_to_string;
use std::collections::HashMap;
use subxt::utils::{AccountId32, H256};

const MARKET: H256 = H256::repeat_byte(0xaa);
const SLOT: u64 = 1_800_000_000;

fn account(byte: u8) -> AccountId32 {
	AccountId32([byte; 32])
}

fn component(area: u8, energy: u64, energy_rate: u64) -> OrderComponent {
	OrderComponent {
		area_uuid: H256::repeat_byte(area),
		market_id: MARKET,
		time_slot: SLOT,
		creation_time: 1_799_990_000,
		energy,
		energy_rate,
	}
}

fn bid(buyer: u8, area: u8, energy: u64, energy_rate: u64) -> Bid {
	Bid { buyer: account(buyer), nonce: 1, bid_component: component(area, energy, energy_rate) }
}

fn offer(seller: u8, area: u8, energy: u64, energy_rate: u64) -> Offer {
	Offer {
		seller: account(seller),
		nonce: 1,
		offer_component: component(area, energy, energy_rate),
	}
}

/// A stored order whose `_id` is its node hash, as the storage relays it.
fn stored(order: Order) -> StoredOrder {
	let hash = match &order {
		Order::Bid(bid) => bid_hash(bid),
		Order::Offer(offer) => offer_hash(offer),
	};
	StoredOrder { id: h256_to_string(hash), order }
}

/// A registry lookup over a fixed map; keys missing from the map have no registry entry.
fn registry(
	entries: Vec<(&StoredOrder, RegistryStatus)>,
) -> impl FnMut(&AccountId32, &H256) -> Option<RegistryStatus> {
	let entries: HashMap<([u8; 32], H256), RegistryStatus> = entries
		.into_iter()
		.map(|(order, status)| ((order.owner().0, order.hash()), status))
		.collect();
	move |owner, hash| entries.get(&(owner.0, *hash)).copied()
}

#[test]
fn keeps_open_orders_and_drops_missing_executed_and_deleted_ones() {
	let open_bid = stored(Order::Bid(bid(1, 1, 10_000, 3_000)));
	let executed_bid = stored(Order::Bid(bid(1, 2, 20_000, 3_000)));
	let open_offer = stored(Order::Offer(offer(2, 3, 10_000, 700)));
	let deleted_offer = stored(Order::Offer(offer(2, 4, 20_000, 700)));
	let unregistered_offer = stored(Order::Offer(offer(2, 5, 30_000, 700)));
	let lookup = registry(vec![
		(&open_bid, RegistryStatus::Open),
		(&executed_bid, RegistryStatus::Executed),
		(&open_offer, RegistryStatus::Open),
		(&deleted_offer, RegistryStatus::Deleted),
	]);

	let filtered = filter_registered_open(
		vec![
			open_bid.clone(),
			executed_bid.clone(),
			open_offer.clone(),
			deleted_offer.clone(),
			unregistered_offer.clone(),
		],
		lookup,
	);

	assert_eq!(filtered.bids, vec![bid(1, 1, 10_000, 3_000)]);
	assert_eq!(filtered.offers, vec![offer(2, 3, 10_000, 700)]);
	let dropped: Vec<(H256, Option<RegistryStatus>)> =
		filtered.dropped.iter().map(|order| (order.hash, order.status)).collect();
	assert_eq!(
		dropped,
		vec![
			(executed_bid.hash(), Some(RegistryStatus::Executed)),
			(deleted_offer.hash(), Some(RegistryStatus::Deleted)),
			(unregistered_offer.hash(), None),
		]
	);
	assert_eq!(filtered.dropped[2].owner, account(2));
	assert_eq!(filtered.dropped[2].id, unregistered_offer.id);
}

#[test]
fn a_missing_registry_entry_is_not_treated_as_open() {
	let unregistered = stored(Order::Bid(bid(1, 1, 10_000, 3_000)));

	let filtered = filter_registered_open(vec![unregistered.clone()], |_, _| None);

	assert!(filtered.bids.is_empty());
	assert_eq!(filtered.dropped.len(), 1);
	assert_eq!(filtered.dropped[0].status, None);
	assert_eq!(filtered.dropped[0].hash, unregistered.hash());
}

#[test]
fn the_registry_is_queried_by_owner_and_node_hash() {
	let order = stored(Order::Offer(offer(7, 1, 10_000, 700)));
	let mut queried = Vec::new();

	filter_registered_open(vec![order.clone()], |owner, hash| {
		queried.push((owner.clone(), *hash));
		Some(RegistryStatus::Open)
	});

	assert_eq!(queried, vec![(account(7), offer_hash(&offer(7, 1, 10_000, 700)))]);
	// The storage `_id` is the same hash, as relayed by the orderbook worker.
	assert_eq!(h256_to_string(queried[0].1), order.id);
}

#[test]
fn the_order_hash_equals_the_storage_id_of_the_relayed_order() {
	use gsy_offchain_primitives::node_to_api_schema::insert_order::{
		convert_gsy_node_order_schema_to_db_schema, Bid as RelayedBid, Offer as RelayedOffer,
		Order as RelayedOrder, OrderComponent as RelayedComponent, OrderSchema,
	};
	use gsy_offchain_primitives::db_api_schema::orders::OrderStatus;
	use codec::Encode;

	let canonical_bid = bid(1, 1, 10_000, 3_000);
	let canonical_offer = offer(2, 3, 20_000, 700);
	let relayed_component = |component: &OrderComponent| RelayedComponent {
		area_uuid: component.area_uuid,
		market_id: component.market_id,
		time_slot: component.time_slot,
		creation_time: component.creation_time,
		energy: component.energy,
		energy_rate: component.energy_rate,
	};
	// The orders as the orderbook worker relays them to the storage.
	let relayed = vec![
		OrderSchema {
			_id: H256::zero(),
			status: OrderStatus::Open,
			order: RelayedOrder::Bid(RelayedBid {
				buyer: canonical_bid.buyer.clone(),
				nonce: canonical_bid.nonce,
				bid_component: relayed_component(&canonical_bid.bid_component),
			}),
		},
		OrderSchema {
			_id: H256::zero(),
			status: OrderStatus::Open,
			order: RelayedOrder::Offer(RelayedOffer {
				seller: canonical_offer.seller.clone(),
				nonce: canonical_offer.nonce,
				offer_component: relayed_component(&canonical_offer.offer_component),
			}),
		},
	];
	let storage_ids: Vec<String> = convert_gsy_node_order_schema_to_db_schema(relayed.encode())
		.into_iter()
		.map(|order| order._id)
		.collect();

	assert_eq!(
		storage_ids,
		vec![h256_to_string(bid_hash(&canonical_bid)), h256_to_string(offer_hash(&canonical_offer))]
	);
}

#[test]
fn the_bid_ranked_behind_an_unregistered_offer_matches_a_valid_offer_instead() {
	// The unregistered offer has the lowest rate, so pay_as_bid would match it first, against the
	// best bid, and settle_trades would reject the proposal every cycle.
	let unregistered_offer = offer(2, 3, 10_000, 100);
	let valid_offer = offer(3, 4, 10_000, 700);
	let best_bid = bid(1, 1, 10_000, 3_000);
	let orders = vec![
		stored(Order::Bid(best_bid.clone())),
		stored(Order::Offer(unregistered_offer.clone())),
		stored(Order::Offer(valid_offer.clone())),
	];

	let unfiltered = MatchingData {
		bids: vec![best_bid.clone()],
		offers: vec![unregistered_offer.clone(), valid_offer.clone()],
		market_id: MARKET,
	}
	.pay_as_bid();
	assert_eq!(unfiltered.len(), 1);
	assert_eq!(unfiltered[0].offer, unregistered_offer, "precondition: it ranks first");

	let lookup = registry(vec![(&orders[0], RegistryStatus::Open), (&orders[2], RegistryStatus::Open)]);
	let filtered = filter_registered_open(orders, lookup);
	let matches =
		MatchingData { bids: filtered.bids, offers: filtered.offers, market_id: MARKET }.pay_as_bid();

	assert_eq!(matches.len(), 1);
	assert_eq!(matches[0].bid, best_bid);
	assert_eq!(matches[0].offer, valid_offer);
	assert_eq!(match_order_hashes(&matches), vec![(bid_hash(&best_bid), offer_hash(&valid_offer))]);
}

fn dropped(byte: u8) -> DroppedOrder {
	DroppedOrder {
		hash: H256::repeat_byte(byte),
		id: format!("id-{}", byte),
		owner: account(byte),
		status: None,
	}
}

#[test]
fn a_dropped_order_is_logged_once_and_forgotten_once_gone() {
	let mut log = DroppedOrderLog::new();
	let hashes = |orders: Vec<&DroppedOrder>| -> Vec<H256> {
		orders.into_iter().map(|order| order.hash).collect()
	};

	assert_eq!(
		hashes(log.newly_dropped(&[dropped(1), dropped(2)])),
		vec![H256::repeat_byte(1), H256::repeat_byte(2)]
	);
	// The next cycle drops the same orders plus a new one: only the new one is reported.
	assert_eq!(
		hashes(log.newly_dropped(&[dropped(1), dropped(2), dropped(3)])),
		vec![H256::repeat_byte(3)]
	);
	// Order 1 is gone from storage's open set: it is forgotten, so the log stays bounded.
	assert!(log.newly_dropped(&[dropped(2), dropped(3)]).is_empty());
	assert_eq!(log.len(), 2);
	// Should order 1 be dropped again later, it is reported again.
	assert_eq!(hashes(log.newly_dropped(&[dropped(1)])), vec![H256::repeat_byte(1)]);
	assert_eq!(log.len(), 1);
}
