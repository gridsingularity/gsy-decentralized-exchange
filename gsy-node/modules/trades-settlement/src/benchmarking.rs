//! Benchmarking setup for trades-settlement
#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[allow(unused)]
use crate::Pallet as TradesSettlement;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller, Vec};
use frame_support::{
	sp_runtime::traits::{Hash, One},
	traits::Currency,
};
use frame_system::RawOrigin;
use gsy_collateral::{BalanceOf, Pallet as GsyCollateral};
use gsy_primitives::{Bid, BidOfferMatch, Offer, Order, OrderComponent, Vault};
use orderbook_registry::Pallet as OrderbookRegistry;
use orderbook_worker::Pallet as OrderbookWorker;
use sp_std::vec;

fn add_user<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
	let _ = GsyCollateral::<T>::add_user(user);
	Ok(())
}

fn add_matching_engine_operator<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
	let _ = GsyCollateral::<T>::add_matching_engine_operator(user);
	Ok(())
}

fn dummy_bid<T: Config>(
	buyer: T::AccountId,
	block_number: u64,
	energy: u64,
	energy_rate: u64,
) -> Bid<T::AccountId> {
	Bid {
		buyer,
		nonce: 1,
		bid_component: OrderComponent {
			area_uuid: 1,
			market_uuid: 1u64,
			time_slot: block_number,
			creation_time: 1677453190,
			energy,
			energy_rate
		},
	}
}

fn dummy_offer<T: Config>(
	seller: T::AccountId,
	block_number: u64,
	energy: u64,
	energy_rate: u64,
) -> Offer<T::AccountId> {
	Offer {
		seller,
		nonce: 1,
		offer_component: OrderComponent {
			area_uuid: 2,
			market_uuid: 1u64,
			time_slot: block_number,
			creation_time: 1677453190,
			energy,
			energy_rate
		},
	}
}

fn dummy_bid_offer_match<T: Config>(
	bid: Bid<T::AccountId>,
	offer: Offer<T::AccountId>,
	residual_bid: Option<Bid<T::AccountId>>,
	residual_offer: Option<Offer<T::AccountId>>,
	block_number: u64,
	selected_energy: u64,
	energy_rate: u64,
) -> BidOfferMatch<T::AccountId> {
	BidOfferMatch {
		market_id: 1,
		time_slot: block_number,
		bid,
		offer,
		residual_offer,
		residual_bid,
		selected_energy,
		energy_rate,
	}
}

benchmarks! {
	settle_trades {
		let matching_engine: T::AccountId = whitelisted_caller();
		add_matching_engine_operator::<T>(matching_engine.clone()).unwrap();
		let buyer: T::AccountId = whitelisted_caller();
		add_user::<T>(buyer.clone()).unwrap();
		let seller: T::AccountId = whitelisted_caller();
		add_user::<T>(seller.clone()).unwrap();
		let block_number = 1677453190;
		let _ = GsyCollateral::<T>::create(matching_engine.clone());
		let _ = GsyCollateral::<T>::create(buyer.clone());
		let _ = GsyCollateral::<T>::create(seller.clone());
		let amount: BalanceOf<T> = 10_000_000u32.into();
		<T as gsy_collateral::Config>::Currency::deposit_creating(&seller, amount * 2u32.into());
		let _ = GsyCollateral::<T>::deposit(&seller, amount);
		let mut bid_offer_matches: Vec<BidOfferMatch<T::AccountId>> = vec![];
		for i in 0..100 {
			let bid = dummy_bid::<T>(buyer.clone(), block_number, i as u32, i as u32);
			let bid_order = Order::Bid(bid.clone());
			let bid_order_hash = T::Hashing::hash_of(&bid_order);
			let _ = OrderbookRegistry::<T>::add_order(buyer.clone(), bid_order_hash.clone());
			let _ = OrderbookWorker::<T>::add_order(buyer.clone(), bid_order.clone());
			let offer = dummy_offer::<T>(seller.clone(), block_number, i as u32, i as u32);
			let offer_order = Order::Offer(offer.clone());
			let offer_order_hash = T::Hashing::hash_of(&offer_order);
			let _ = OrderbookRegistry::<T>::add_order(seller.clone(), offer_order_hash.clone());
			let _ = OrderbookWorker::<T>::add_order(seller.clone(), offer_order.clone());
			let bid_offer_match = dummy_bid_offer_match::<T>(
				bid.clone(),
				offer.clone(),
				None,
				None,
				block_number,
				i as u32,
				i as u32,
			);
			bid_offer_matches.push(bid_offer_match);
		}
	}: _(RawOrigin::Signed(matching_engine.clone()), bid_offer_matches)
}

impl_benchmark_test_suite!(TradesSettlement, crate::mock::new_test_ext(), crate::mock::Test);
