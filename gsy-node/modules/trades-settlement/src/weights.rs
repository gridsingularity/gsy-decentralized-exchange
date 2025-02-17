#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait TradeSettlementWeightInfo {
	fn settle_trades() -> Weight;
	fn submit_penalties() -> Weight;
}

/// Weight functions for `trades_settlement`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> TradeSettlementWeightInfo for SubstrateWeightInfo<T> {
	// Storage: GsyCollateral RegisteredExchangeOperator (r:1 w:0)
	// Storage: OrderbookRegistry OrdersRegistry (r:200 w:200)
	// Storage: GsyCollateral Vaults (r:1 w:1)
	// Storage: OrderbookRegistry TradesRegistry (r:0 w:1)
	fn settle_trades() -> Weight {
		Weight::from_parts(5_586_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(202))
			.saturating_add(T::DbWeight::get().writes(202))
	}

	fn submit_penalties() -> Weight {
        Weight::from_parts(10_000_000, 0)
            .saturating_add(T::DbWeight::get().writes(202))
    }
}
