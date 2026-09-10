//! Benchmarking setup for remuneration
#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[allow(unused)]
use crate::Pallet as Remuneration;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller, Vec};
use frame_support::{
	sp_runtime::traits::{Hash, One},
	traits::Currency,
};
use frame_system::RawOrigin;
use gsy_collateral::{BalanceOf, Pallet as GsyCollateral};
use gsy_primitives::{Bid, BidOfferMatch, Offer, Order, OrderComponent, Vault};
use orderbook_registry::Pallet as OrderbookRegistry;
use orderbook_worker::Pallet as OrderbookWorker;
use sp_std::vec;
