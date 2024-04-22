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
}

/// Weight functions for `orderbook_worker`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn insert_orders() -> Weight {
		(2_495_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(101 as Weight))
			.saturating_add(T::DbWeight::get().writes(200 as Weight))
	}
	// Storage: GsyCollateral ProxyAccounts (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn insert_orders_by_proxy() -> Weight {
		(2_582_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(101 as Weight))
			.saturating_add(T::DbWeight::get().writes(200 as Weight))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn remove_orders() -> Weight {
		(2_675_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(101 as Weight))
			.saturating_add(T::DbWeight::get().writes(200 as Weight))
	}
	// Storage: GsyCollateral ProxyAccounts (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:100 w:100)
	// Storage: OrderbookWorker Orderbook (r:0 w:100)
	fn remove_orders_by_proxy() -> Weight {
		(2_724_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(101 as Weight))
			.saturating_add(T::DbWeight::get().writes(200 as Weight))
	}
}
