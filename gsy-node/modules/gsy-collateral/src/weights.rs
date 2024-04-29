#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn deposit_collateral() -> Weight;
	fn register_proxy_account() -> Weight;
	fn register_matching_engine_operator() -> Weight;
	fn register_user() -> Weight;
	fn restart_vault() -> Weight;
	fn shutdown_vault() -> Weight;
	fn unregister_proxy_account() -> Weight;
	fn withdraw_collateral() -> Weight;
}
/// Weight functions for `gsy_collateral`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: GsyCollateral Vaults (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn deposit_collateral() -> Weight {
		Weight::from_parts(29_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: GsyCollateral ProxyAccounts (r:1 w:1)
	fn register_proxy_account() -> Weight {
		Weight::from_parts(19_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: GsyCollateral RegisteredMatchingEngine (r:1 w:1)
	fn register_matching_engine_operator() -> Weight {
		Weight::from_parts(20_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:1)
	// Storage: GsyCollateral VaultCount (r:1 w:1)
	// Storage: GsyCollateral Vaults (r:0 w:1)
	fn register_user() -> Weight {
		Weight::from_parts(29_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	// Storage: GsyCollateral Vaults (r:1 w:1)
	fn restart_vault() -> Weight {
		Weight::from_parts(17_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: GsyCollateral Vaults (r:1 w:1)
	fn shutdown_vault() -> Weight {
		Weight::from_parts(18_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: GsyCollateral ProxyAccounts (r:1 w:1)
	fn unregister_proxy_account() -> Weight {
		Weight::from_parts(21_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: GsyCollateral RegisteredUser (r:1 w:0)
	// Storage: GsyCollateral Vaults (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn withdraw_collateral() -> Weight {
		Weight::from_parts(49_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
}
