#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait RemunerationWeightInfo {
	fn update_custodian() -> Weight;
	fn add_community() -> Weight;
	fn remove_community() -> Weight;
	fn add_prosumer() -> Weight;
	fn remove_prosumer() -> Weight;
	fn add_payment() -> Weight;
	fn update_prosumer() -> Weight;
}

/// Weight functions for `remuneration`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> RemunerationWeightInfo for SubstrateWeightInfo<T> {
	fn update_custodian() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn add_community() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn remove_community() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn add_prosumer() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn remove_prosumer() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn add_payment() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn update_prosumer() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}
