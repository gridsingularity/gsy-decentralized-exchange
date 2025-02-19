//! Benchmarking setup for trades-settlement
#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::test_orders::TestOrderbookFunctions;
#[allow(unused)]
use crate::Pallet as TradesSettlement;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use crate::benchmarking::vec::Vec;
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


benchmarks! {
	settle_trades {
		let matching_engine: T::AccountId = whitelisted_caller();
		TestOrderbookFunctions::add_matching_engine_operator::<T>(matching_engine.clone()).unwrap();
		let buyer: T::AccountId = whitelisted_caller();
		TestOrderbookFunctions::add_user::<T>(buyer.clone()).unwrap();
		let seller: T::AccountId = whitelisted_caller();
		TestOrderbookFunctions::add_user::<T>(seller.clone()).unwrap();
		let block_number = 1677453190;
		let _ = GsyCollateral::<T>::create(matching_engine.clone());
		let _ = GsyCollateral::<T>::create(buyer.clone());
		let _ = GsyCollateral::<T>::create(seller.clone());
		let amount: BalanceOf<T> = 10_000_000u32.into();
		<T as gsy_collateral::Config>::Currency::deposit_creating(&seller, amount * 2u32.into());
		let _ = GsyCollateral::<T>::deposit(&seller, amount);
		let mut bid_offer_matches: Vec<BidOfferMatch<T::AccountId>> = vec![];
		for i in 0..100 {
			let bid = TestOrderbookFunctions::dummy_bid::<T>(
				buyer.clone(), block_number, i as u64, i as u64);
			let bid_order = Order::Bid(bid.clone());
			let bid_order_hash = T::Hashing::hash_of(&bid_order);
			let _ = OrderbookRegistry::<T>::insert_orders(RawOrigin::Signed(buyer.clone()).into(), vec!(bid_order_hash.clone()));
			let _ = OrderbookWorker::<T>::add_order(buyer.clone(), bid_order.clone());
			let offer = TestOrderbookFunctions::dummy_offer::<T>(
				seller.clone(), block_number, i as u64, i as u64);
			let offer_order = Order::Offer(offer.clone());
			let offer_order_hash = T::Hashing::hash_of(&offer_order);
			let _ = OrderbookRegistry::<T>::insert_orders(RawOrigin::Signed(seller.clone()).into(), vec!(offer_order_hash.clone()));
			let _ = OrderbookWorker::<T>::add_order(seller.clone(), offer_order.clone());
			let bid_offer_match = TestOrderbookFunctions::dummy_bid_offer_match::<T>(
				bid.clone(),
				offer.clone(),
				None,
				None,
				block_number,
				i as u64,
				i as u64,
			);
			bid_offer_matches.push(bid_offer_match);
		}
	}: _(RawOrigin::Signed(matching_engine.clone()), bid_offer_matches)
}

impl_benchmark_test_suite!(TradesSettlement, crate::mock::new_test_ext(), crate::mock::Test);
