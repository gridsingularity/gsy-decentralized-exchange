#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait RemunerationWeightInfo {
	fn update_custodian() -> Weight;
	fn update_custodian_gsy() -> Weight;
	fn add_community() -> Weight;
	fn remove_community() -> Weight;
	fn add_prosumer() -> Weight;
	fn remove_prosumer() -> Weight;
	fn add_payment() -> Weight;
	fn update_prosumer() -> Weight;
	fn set_balance() -> Weight;
	fn set_main_parameters() -> Weight; // consolidated alpha/beta/tolerances
	fn settle_flexibility_payment() -> Weight;
	fn set_adaptation_params() -> Weight;
	fn dynamically_adapt_parameters() -> Weight;
	fn set_piecewise_parameters() -> Weight;
	fn settle_flexibility_payment_with_pw_quad_penalty() -> Weight;
}

/// Weight functions for `remuneration`.
pub struct SubstrateWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> RemunerationWeightInfo for SubstrateWeightInfo<T> {
	fn update_custodian() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn update_custodian_gsy() -> Weight {
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
	fn set_balance() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn set_main_parameters() -> Weight {
		Weight::from_parts(9_000_000, 0)
			// writes alpha, beta, under_tolerance, over_tolerance
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	fn settle_flexibility_payment() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn set_adaptation_params() -> Weight {
		Weight::from_parts(9_000_000, 0)
			// write u_ref, o_ref, k_alpha, k_beta, k_under_tol, window size
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	fn dynamically_adapt_parameters() -> Weight {
		Weight::from_parts(9_000_000, 0)
			// writes alpha, beta, under_tolerance
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	fn set_piecewise_parameters() -> Weight {
		Weight::from_parts(9_000_000, 0)
			// writes alpha_pw, eps1, eps2
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	fn settle_flexibility_payment_with_pw_quad_penalty() -> Weight {
		Weight::from_parts(9_000_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}
