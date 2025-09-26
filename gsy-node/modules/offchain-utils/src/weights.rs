#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn create_job() -> Weight;
	fn submit_result_unsigned() -> Weight;
}

/// Weight functions for `offchain-utils`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeightInfo<T> {
	fn create_job() -> Weight {
		// Storage:
		// - Jobs: read (to ensure not exists), write (insert)
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn submit_result_unsigned() -> Weight {
		// Storage:
		// - Jobs: read (to ensure exists), write (remove)
		// - Results: read (to ensure not exists), write (insert)
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}

