//! Benchmarking setup for gsy-collateral
#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[allow(unused)]
use crate::Pallet as GsyCollateral;
use gsy_primitives::{Vault, VaultWithStatus};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::{EventRecord, RawOrigin};
use frame_support::traits::Currency;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	let events = frame_system::Pallet::<T>::events();
	let system_event: <T as frame_system::Config>::Event = generic_event.into();
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

fn add_user<T: Config>(
	user: T::AccountId,
) -> Result<(), &'static str> {
	let _ = GsyCollateral::<T>::add_user(user);
	Ok(())
}

fn add_proxy_account<T: Config>(
	delegator: &T::AccountId,
	proxy_account: T::AccountId,
) -> Result<(), &'static str> {
	let _ = GsyCollateral::<T>::add_proxy_account(delegator, proxy_account);
	Ok(())
}

benchmarks! {
	deposit_collateral {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let _ = GsyCollateral::<T>::create(caller.clone());
		let amount: BalanceOf<T> = 10_000_000u32.into();
		T::Currency::deposit_creating(&caller, amount * 2u32.into());
	}: _(RawOrigin::Signed(caller.clone()), amount)
	verify {
		assert_last_event::<T>(Event::CollateralDeposited(
			caller.clone(),
			amount,
		).into());
	}

	register_proxy_account {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let proxy_account: T::AccountId = T::AccountId::default();
	}: _(RawOrigin::Signed(caller.clone()), proxy_account.clone())
	verify {
		assert_last_event::<T>(Event::ProxyAccountRegistered(
			caller.clone(),
			proxy_account.clone(),
		).into());
	}

	register_matching_engine_operator {
		let matching_engine_operator: T::AccountId = whitelisted_caller();
	}: _(RawOrigin::Root, matching_engine_operator.clone())
	verify {
		assert_last_event::<T>(Event::MatchingEngineOperatorRegistered(
			matching_engine_operator.clone(),
		).into());
	}

	register_user {
		let user: T::AccountId = whitelisted_caller();
		let id = 1.into();
	}: _(RawOrigin::Root, user.clone())
	verify {
		assert_last_event::<T>(Event::VaultCreated(
			id,
			user.clone()
		).into());
	}

	restart_vault {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let _ = GsyCollateral::<T>::create(caller.clone());
		let _ = GsyCollateral::<T>::freeze(&caller);
	}: _(RawOrigin::Root, caller.clone())
	verify {
		assert_last_event::<T>(Event::VaultRestarted(
			caller.clone(),
		).into());
	}

	shutdown_vault {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let _ = GsyCollateral::<T>::create(caller.clone());
	}: _(RawOrigin::Root, caller.clone())
	verify {
		assert_last_event::<T>(Event::VaultShutdown(
			caller.clone(),
		).into());
	}

	unregister_proxy_account {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let proxy_account: T::AccountId = T::AccountId::default();
		add_proxy_account::<T>(&caller, proxy_account.clone()).unwrap();
	}: _(RawOrigin::Signed(caller.clone()), proxy_account.clone())
	verify {
		assert_last_event::<T>(Event::ProxyAccountUnregistered(
			caller.clone(),
			proxy_account.clone(),
		).into());
	}

	withdraw_collateral {
		let caller: T::AccountId = whitelisted_caller();
		add_user::<T>(caller.clone()).unwrap();
		let _ = GsyCollateral::<T>::create(caller.clone());
		let amount: BalanceOf<T> = 10_000_000u32.into();
		T::Currency::deposit_creating(&caller, amount * 2u32.into());
		let _ = GsyCollateral::<T>::deposit(&caller, amount);
		let withdraw_amount: BalanceOf<T> = 9_000_000u32.into();
	}: _(RawOrigin::Signed(caller.clone()), withdraw_amount)
	verify {
		assert_last_event::<T>(Event::CollateralWithdrawn(
			caller.clone(),
			withdraw_amount,
		).into());
	}
}

impl_benchmark_test_suite!(
	GsyCollateral,
	crate::mock::new_test_ext(),
	crate::mock::Test
);
