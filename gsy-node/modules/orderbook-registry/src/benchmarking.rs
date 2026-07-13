//! Benchmarking setup for orderbook-registry. Empty for now

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};

benchmarks! {
	empty_benchmark {
		let caller: T::AccountId = whitelisted_caller();
	}: {
		// TODO: Implement benchmarking for orderbook registry
	}
}
