//! Benchmarking setup for orderbook-worker
#![cfg(feature = "runtime-benchmarks")]

#[allow(unused)]
use crate::Pallet as OrderbookWorker;

// fn add_user<T: Config>(
// 	user: T::AccountId,
// ) -> Result<(), &'static str> {
// 	let _ = GsyCollateral::<T>::add_user(user);
// 	Ok(())
// }
//
// fn add_proxy_account<T: Config>(
// 	delegator: &T::AccountId,
// 	proxy_account: T::AccountId,
// ) -> Result<(), &'static str> {
// 	let _ = GsyCollateral::<T>::add_proxy_account(delegator, proxy_account);
// 	Ok(())
// }
//
// fn dummy_bid<T: Config>(
// 	buyer: T::AccountId,
// 	nonce: u32,
// 	block_number: u64,
// 	energy: u64,
// 	energy_rate: u64
// ) -> Bid<T::AccountId> {
// 	Bid {
// 		buyer,
// 		nonce,
// 		bid_component: OrderComponent {
// 			area_uuid: 1,
// 			market_id: 1u64,
// 			time_slot: block_number,
// 			creation_time: 1677453190,
// 			energy,
// 			energy_rate,
// 		},
// 	}
// }
//
// benchmarks! {
// 	insert_orders {
// 		let caller: T::AccountId = whitelisted_caller();
// 		add_user::<T>(caller.clone()).unwrap();
// 		let block_number = 1677453190;
// 		let _ = GsyCollateral::<T>::create(caller.clone());
// 		let mut orders: Vec<Order<T::AccountId>> = vec![];
// 		for i in 0..100 {
// 			let bid = dummy_bid::<T>(caller.clone(), i as u32 + 1u32, block_number, i as u32, i as u32);
// 			orders.push(Order::Bid(bid.clone()));
// 		}
// 	}: _(RawOrigin::Signed(caller.clone()), orders)
//
// 	insert_orders_by_proxy {
// 		let delegator: T::AccountId = T::AccountId::default();
// 		let proxy_account: T::AccountId = whitelisted_caller();
// 		add_user::<T>(delegator.clone()).unwrap();
// 		add_proxy_account::<T>(&delegator, proxy_account.clone()).unwrap();
// 		let block_number = 1677453190;
// 		let _ = GsyCollateral::<T>::create(delegator.clone());
// 		let mut orders: Vec<Order<T::AccountId>> = vec![];
// 		for i in 0..100 {
// 			let bid = dummy_bid::<T>(delegator.clone(), i as u32 + 1u32, block_number, i as u32, i as u32);
// 			orders.push(Order::Bid(bid.clone()));
// 		}
// 	}: _(RawOrigin::Signed(proxy_account.clone()), delegator.clone(), orders)
//
// 	remove_orders {
// 		let caller: T::AccountId = whitelisted_caller();
// 		add_user::<T>(caller.clone()).unwrap();
// 		let block_number = 1677453190;
// 		let _ = GsyCollateral::<T>::create(caller.clone());
// 		let mut orders: Vec<Order<T::AccountId>> = vec![];
// 		for i in 0..100 {
// 			let bid = dummy_bid::<T>(caller.clone(), i as u32 + 1u32, block_number, i as u32, i as u32);
// 			let order = Order::Bid(bid.clone());
// 			orders.push(order.clone());
// 			let order_hash = T::Hashing::hash_of(&order);
// 			let _ = OrderbookRegistry::<T>::add_order(caller.clone(), order_hash.clone());
// 			let _ = OrderbookWorker::<T>::add_order(caller.clone(), order.clone());
// 		}
// 	}: _(RawOrigin::Signed(caller.clone()), orders)
//
// 	remove_orders_by_proxy {
// 		let delegator: T::AccountId = T::AccountId::default();
// 		let proxy_account: T::AccountId = whitelisted_caller();
// 		add_user::<T>(delegator.clone()).unwrap();
// 		add_proxy_account::<T>(&delegator, proxy_account.clone()).unwrap();
// 		let block_number = 1677453190;
// 		let _ = GsyCollateral::<T>::create(delegator.clone());
// 		let mut orders: Vec<Order<T::AccountId>> = vec![];
// 		for i in 0..100 {
// 			let bid = dummy_bid::<T>(delegator.clone(), i as u32 + 1u32, block_number, i as u32, i as u32);
// 			let order = Order::Bid(bid.clone());
// 			orders.push(order.clone());
// 			let order_hash = T::Hashing::hash_of(&order);
// 			let _ = OrderbookRegistry::<T>::add_order(delegator.clone(), order_hash.clone());
// 			let _ = OrderbookWorker::<T>::add_order(delegator.clone(), order.clone());
// 		}
// 	}: _(RawOrigin::Signed(proxy_account.clone()), delegator.clone(), orders)
// }
//
// impl_benchmark_test_suite!(
// 	OrderbookWorker,
// 	crate::mock::new_test_ext(),
// 	crate::mock::Test
// );
