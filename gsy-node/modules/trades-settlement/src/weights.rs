#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn settle_trades() -> Weight;
}

/// Weight functions for `trades_settlement`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	// Storage: GsyCollateral RegisteredMatchingEngine (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:200 w:200)
	// Storage: GsyCollateral Vaults (r:1 w:1)
	// Storage: OrderbookRegistry TradesRegistry (r:0 w:1)
	fn settle_trades() -> Weight {
		(5_586_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(202 as Weight))
			.saturating_add(T::DbWeight::get().writes(202 as Weight))
	}
}
