#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn insert_orders() -> Weight;
	fn insert_orders_by_proxy() -> Weight;
	fn remove_orders() -> Weight;
	fn remove_orders_by_proxy() -> Weight;
	fn zero_weight() -> Weight;
}

/// Weight functions for `orderbook_worker`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn insert_orders() -> Weight {
		Weight::from_parts(2_495_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(101))
			.saturating_add(T::DbWeight::get().writes(200))
	}
	// Storage: GsyCollateral ProxyAccounts (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn insert_orders_by_proxy() -> Weight {
		Weight::from_parts(2_582_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(101))
			.saturating_add(T::DbWeight::get().writes(200))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn remove_orders() -> Weight {
		Weight::from_parts(2_675_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(101))
			.saturating_add(T::DbWeight::get().writes(200))
	}
	// Storage: GsyCollateral ProxyAccounts (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn remove_orders_by_proxy() -> Weight {
		Weight::from_parts(2_724_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(101))
			.saturating_add(T::DbWeight::get().writes(200))
	}

	// zero weight method for calls that do not require weight
	fn zero_weight() -> Weight {
		Weight::from_parts(0, 0)
			.saturating_add(T::DbWeight::get().reads(101))
			.saturating_add(T::DbWeight::get().writes(200))
	}
}
