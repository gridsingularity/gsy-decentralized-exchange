#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait TradeSettlementWeightInfo {
	fn settle_trades() -> Weight;
	fn set_energy_to_money_factor() -> Weight;
	fn submit_penalties(p: u32, e: u32) -> Weight;
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
	fn set_energy_to_money_factor() -> Weight {
		Weight::from_parts(5_586_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(202))
			.saturating_add(T::DbWeight::get().writes(202))
	}

	// Hand-tuned estimate, not benchmark output: a base cost plus a per-penalty and
	// per-evaluated-uuid component, and one `PenaltiesRegistry` write per penalty.
	fn submit_penalties(p: u32, e: u32) -> Weight {
		Weight::from_parts(10_000_000, 0)
			.saturating_add(Weight::from_parts(2_000_000, 0).saturating_mul(p.into()))
			.saturating_add(Weight::from_parts(500_000, 0).saturating_mul(e.into()))
			.saturating_add(T::DbWeight::get().writes(p.into()))
	}
}
