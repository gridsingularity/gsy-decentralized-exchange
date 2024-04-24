// This file is part of GSy-Decentralized Energy Exchange.

// Copyright (C) Grid Singularity Gmbh.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Collateral Management System ( gsy-collateral )
//!
//!
//! A collateral management system is a system that manages the collateral of a registered user
//! in the GSy-Decentralized Energy Exchange. This module allows the user to deposit a collateral
//! and withdraw it from the system. Moreover it allows the registered user to add or remove proxy
//!	accounts which can insert order on behalf of the registered user.
//! It also allows the root user to register new users allowing them to deposit collateral.


#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{FullCodec, MaxEncodedLen};
	use core::ops::AddAssign;
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		sp_runtime::DispatchError
	};
	use frame_support::{
		require_transactional,
		sp_runtime::traits::Hash,
		traits::{tokens::ExistenceRequirement, Currency},
		transactional, PalletId, sp_runtime::SaturatedConversion
	};
	use core::fmt::Debug;
	use frame_system::pallet_prelude::*;
	use gsy_primitives::v0::{CollateralInfo, Vault, VaultInfo, VaultStatus, VaultWithStatus};
	use num_traits::{ One, Zero};
	use scale_info::TypeInfo;
	use crate::weights::CollateralWeightInfo;

	pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	// The proxy struct for the pallet.
	#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	RuntimeDebug,
	MaxEncodedLen,
	TypeInfo,
	)]
	pub struct ProxyDefinition<AccountId> {
		// The account which may act as proxy.
		pub proxy: AccountId,
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The Currency handler for the gsy-collateral pallet.
		type Currency: Currency<Self::AccountId>;

		/// The maximum number of proxy account a registered user can have.
		#[pallet::constant]
		type ProxyAccountLimit: Get<u32>;

		/// The id used as `AccountId` for the vault.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Key type for the vaults. `VaultId` uniquely identifies a vault. The identifiers are
		type VaultId: AddAssign
		+ FullCodec
		+ One
		+ Eq
		+ PartialEq
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default
		+ TypeInfo
		+ MaxEncodedLen
		+ Encode
		+ Into<u128>
		+ From<u64>;

		type CollateralWeightInfo: CollateralWeightInfo;
	}

	#[pallet::pallet]
	// #[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// Storage items.

	#[pallet::storage]
	#[pallet::getter(fn registered_user)]
	/// Keeps track of the registered user.
	pub type RegisteredUser<T: Config> =
	StorageMap<_, Twox64Concat, T::AccountId, T::Hash, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn registered_matching_engine)]
	/// Keeps track of the registered user.
	pub type RegisteredMatchingEngine<T: Config> =
	StorageMap<_, Twox64Concat, T::AccountId, T::Hash, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn proxy_accounts)]
	/// Keeps track of the proxy accounts for each registered user.
	pub type ProxyAccounts<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<ProxyDefinition<T::AccountId>, T::ProxyAccountLimit>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn vault_count)]
	/// The number of vaults, also used to generate the next vault identifier.
	pub type VaultCount<T: Config> = StorageValue<_, T::VaultId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn vault_data)]
	/// Keeps track of the vault data.
	pub type Vaults<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		VaultInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::VaultId>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New collateral has been deposited. \[depositor, amount\]
		CollateralDeposited(T::AccountId, BalanceOf<T>),
		/// Collateral has been withdrawn. \[depositor, amount\]
		CollateralWithdrawn(T::AccountId, BalanceOf<T>),
		/// User has registered a proxy account. \[user_account, proxy_account\]
		ProxyAccountRegistered(T::AccountId, T::AccountId),
		/// User has unregistered a proxy account. \[user_account, proxy_account\]
		ProxyAccountUnregistered(T::AccountId, T::AccountId),
		/// User has been registered. \[user_account\]
		UserRegistered(T::AccountId),
		/// Matching Engine operator has been registered. \[matching_engine_operator\]
		MatchingEngineOperatorRegistered(T::AccountId),
		/// New vault has been created. \[vault_id, vault_owner\]
		VaultCreated(T::VaultId, T::AccountId),
		/// A Vault has been successfully restarted. \[vault_owner\]
		VaultRestarted(T::AccountId),
		/// A Vault has been successfully shutdown. \[vault_owner\]
		VaultShutdown(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Cannot register an account already registered.
		AlreadyRegistered,
		/// Cannot register a proxy account already registered.
		AlreadyRegisteredProxyAccount,
		/// Cannot deposit to a vault that is not active.
		DepositsNotAllowed,
		/// Cannot transfer between vaults that are not both active.
		InactiveVault,
		/// Cannot register a self proxy
		NoSelfProxy,
		/// Ensure that the account is a registered user.
		NotARegisteredUserAccount,
		/// Ensure that the account is a registered matching_engine operator.
		NotARegisteredMatchingEngineOperator,
		/// Ensure that the account is a proxy account.
		NotARegisteredProxyAccount,
		/// Ensure that the user has registered some proxy accounts.
		NotRegisteredProxyAccounts,
		/// Ensures that an account has enough funds to deposit as collateral.
		NotEnoughBalance,
		/// Ensure that the collateral in the vault is not less than the withdrawal amount.
		NotEnoughCollateral,
		/// Ensure that the collateral in the vault is not less than the withdrawal amount + transfer fee.
		NotEnoughCollateralForFee,
		/// An account cannot have more proxy than `ProxyAccountLimit`.
		ProxyAccountsLimitReached,
		/// Cannot transfer funds from user to vault and vice-versa.
		TransferFailed,
		/// Ensure that the vault in not closed.
		VaultClosed,
		/// Ensure that a vault owned by the user exists.
		VaultDoesNotExist,
		/// Ensure that the vault is closed.
		VaultNotClosed,
		/// Cannot withdraw collateral from a vault that is not active.
		WithdrawalsNotAllowed,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Deposit a given amount to the collateral vault.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. Account that is depositing the collateral.
		/// * `amount`: The amount of collateral to deposit.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::deposit_collateral())]
		#[pallet::call_index(0)]
		pub fn deposit_collateral(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let depositor = ensure_signed(origin)?;
			// Verify that the depositor is a registered account.
			ensure!(Self::is_registered_user(&depositor), <Error<T>>::NotARegisteredUserAccount);
			log::info!("Depositing collateral for user: {:?} with amount: {:?}", depositor, amount);
			let balance = <Self as Vault>::deposit(&depositor, amount)?;
			Self::deposit_event(Event::CollateralDeposited(depositor, balance));
			Ok(())
		}

		/// Register a proxy account for a given registered user.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The user account that is registering the proxy account.
		/// * `proxy_account`: The proxy account that is being registered.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::register_proxy_account())]
		#[pallet::call_index(1)]
		pub fn register_proxy_account(
			origin: OriginFor<T>,
			proxy_account: T::AccountId,
		) -> DispatchResult {
			let user_account = ensure_signed(origin)?;
			log::info!(
				"Registering proxy account: {:?} for user: {:?} ",
				proxy_account,
				user_account
			);
			Self::add_proxy_account(&user_account, proxy_account)
		}

		/// Register a matching_engine operator account in the System.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The root user.
		/// * `matching_engine_operator_account`: The matching_engine operator account that is being registered.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::register_matching_engine_operator())]
		#[pallet::call_index(2)]
		pub fn register_matching_engine_operator(
			origin: OriginFor<T>,
			matching_engine_operator_account: T::AccountId,
		) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!("Registering matching_engine operator account: {:?}", matching_engine_operator_account);
			Self::add_matching_engine_operator(matching_engine_operator_account)
		}

		/// Register a new user in the System. (Only the root user can register a new user)
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The root user.
		/// * `user_account`: The account of the new user.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::register_user())]
		#[pallet::call_index(3)]
		pub fn register_user(origin: OriginFor<T>, user_account: T::AccountId) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!("Registering user - {:?} ", user_account);
			Self::add_user(user_account.clone())?;
			let id = <Self as Vault>::create(user_account.clone())?;
			Self::deposit_event(Event::VaultCreated(id, user_account));
			Ok(())
		}

		/// Restart a user vault after shutdown.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The root user.
		/// * `user_account`: The account of the user that is restarting the vault.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::restart_vault())]
		#[pallet::call_index(4)]
		pub fn restart_vault(origin: OriginFor<T>, user_account: T::AccountId) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!("Restarting vault for user - {:?} ", user_account);
			<Self as VaultWithStatus>::unfreeze(&user_account)?;
			Self::deposit_event(Event::VaultRestarted(user_account));
			Ok(())
		}

		/// Stop a user vault. To be used in case of emergency.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The root user.
		/// * `user_account`: The user account that owns the vault.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::shutdown_vault())]
		#[pallet::call_index(5)]
		pub fn shutdown_vault(origin: OriginFor<T>, user_account: T::AccountId) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!("Shutting down the vault for user - {:?} ", user_account);
			<Self as VaultWithStatus>::freeze(&user_account)?;
			Self::deposit_event(Event::VaultShutdown(user_account));
			Ok(())
		}

		/// Unregister a proxy account for a given registered user.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The user account that is unregistering the proxy account.
		/// * `proxy_account`: The proxy account that is being unregistered.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::unregister_proxy_account())]
		#[pallet::call_index(6)]
		pub fn unregister_proxy_account(
			origin: OriginFor<T>,
			proxy_account: T::AccountId,
		) -> DispatchResult {
			let user_account = ensure_signed(origin)?;
			log::info!(
				"Unregistering proxy account: {:?} for user: {:?} ",
				proxy_account,
				user_account
			);
			Self::remove_proxy_account(&user_account, proxy_account)
		}

		/// Withdraw a given amount of collateral from the Vault.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The user account that is withdrawing the collateral.
		/// * `amount`: The amount of collateral to be withdrawn.
		#[transactional]
		#[pallet::weight(<T as Config>::CollateralWeightInfo::withdraw_collateral())]
		#[pallet::call_index(7)]
		pub fn withdraw_collateral(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let user_account = ensure_signed(origin)?;
			// Verify that the depositor is a registered account.
			ensure!(Self::is_registered_user(&user_account), <Error<T>>::NotARegisteredUserAccount);
			log::info!("Withdrawing collateral: {:?} for user: {:?} ", amount, user_account);
			let balance = <Self as Vault>::withdraw(&user_account, amount)?;
			Self::deposit_event(Event::CollateralWithdrawn(user_account, balance));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Register a new matching_engine operator account in the System.
		///
		/// # Parameters:
		/// * `matching_engine_operator_account`: The matching_engine operator account that is being registered.
		pub fn add_matching_engine_operator(matching_engine_operator_account: T::AccountId) -> DispatchResult {
			ensure!(
				!Self::is_registered_matching_engine_operator(&matching_engine_operator_account),
				<Error<T>>::AlreadyRegistered
			);
			let account_hash = T::Hashing::hash_of(&matching_engine_operator_account);
			log::info!("Account Hash - {:?} ", account_hash);
			<RegisteredMatchingEngine<T>>::insert(&matching_engine_operator_account, account_hash);
			// Deposit the MatchingEngineOperatorRegistered event.
			Self::deposit_event(Event::MatchingEngineOperatorRegistered(matching_engine_operator_account));
			Ok(())
		}

		/// Register a new user in the System.
		///
		/// Parameters:
		/// * `user_account`: The account of the new user.
		pub fn add_user(user_account: T::AccountId) -> DispatchResult {
			// Verify that the user is not already registered.
			ensure!(!Self::is_registered_user(&user_account), <Error<T>>::AlreadyRegistered);
			// Register the user.
			let account_hash = T::Hashing::hash_of(&user_account);
			log::info!("Account Hash - {:?} ", account_hash);
			<RegisteredUser<T>>::insert(&user_account, account_hash);
			// Deposit the UserRegistered event.
			Self::deposit_event(Event::UserRegistered(user_account));
			Ok(())
		}

		/// Register a proxy account for a given registered user.
		///
		/// Parameters:
		/// - `delegator`: The origin of the extrinsic. The user account that is registering the proxy account.
		/// - `proxy_account`: The proxy account that is being registered.
		pub fn add_proxy_account(
			delegator: &T::AccountId,
			proxy_account: T::AccountId,
		) -> DispatchResult {
			// Verify that the delegator is not registering itself as proxy.
			ensure!(delegator != &proxy_account, <Error<T>>::NoSelfProxy);
			// Verify that the delegator is a registered account.
			ensure!(Self::is_registered_user(delegator), <Error<T>>::NotARegisteredUserAccount);
			// Add the account to the proxy account storage.
			ProxyAccounts::<T>::try_mutate(delegator, |ref mut proxy_accounts| {
				let proxy_definition = ProxyDefinition { proxy: proxy_account.clone() };
				let i = proxy_accounts
					.binary_search(&proxy_definition)
					.err()
					.ok_or(<Error<T>>::AlreadyRegisteredProxyAccount)?;
				proxy_accounts
					.try_insert(i, proxy_definition)
					.map_err(|_| <Error<T>>::ProxyAccountsLimitReached)?;
				Self::deposit_event(Event::ProxyAccountRegistered(
					delegator.clone(),
					proxy_account,
				));
				Ok(())
			})
		}

		/// Create a new vault for the new registered user.
		///
		/// Parameters:
		/// - `user_account`: The account of the new user.
		pub fn create_vault(
			user_account: T::AccountId,
		) -> Result<
			(T::VaultId, VaultInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::VaultId>),
			DispatchError,
		> {
			VaultCount::<T>::try_mutate(|id| {
				let vault_id = {
					*id += One::one();
					*id
				};

				let vault_info = VaultInfo {
					owner: user_account.clone(),
					id: vault_id.clone(),
					collateral: CollateralInfo {
						amount: Zero::zero(),
						deposit_time: <frame_system::Pallet<T>>::block_number(),
					},
					status: VaultStatus::default(),
				};
				<Vaults<T>>::insert(&user_account, vault_info.clone());
				Ok((vault_id, vault_info))
			})
		}

		/// Deposit collateral to a vault.
		///
		/// Parameters:
		/// - `user_account`: The account of the user that is depositing the collateral.
		/// - `collateral_amount`: The amount of collateral that is being deposited.
		pub fn do_deposit_collateral(
			user_account: &T::AccountId,
			collateral_amount: BalanceOf<T>,
		) -> Result<BalanceOf<T>, DispatchError> {
			let vault_info = Self::vault_info(user_account)?;

			// Verify that the depositor has enough balance to deposit the collateral.
			ensure!(
				T::Currency::free_balance(user_account) >= collateral_amount,
				<Error<T>>::NotEnoughBalance
			);

			ensure!(vault_info.status.are_deposits_allowed(), <Error<T>>::DepositsNotAllowed);

			let to = <Self as Vault>::account_id(&vault_info.id);

			T::Currency::transfer(
				user_account,
				&to,
				collateral_amount,
				ExistenceRequirement::KeepAlive,
			)
				.map_err(|_| <Error<T>>::TransferFailed)?;

			let collateral_info = vault_info.collateral;
			let deposit_time = <frame_system::Pallet<T>>::block_number();
			let new_collateral_info =
				CollateralInfo { amount: collateral_info.amount + collateral_amount, deposit_time };
			let new_vault_info = VaultInfo { collateral: new_collateral_info, ..vault_info };
			<Vaults<T>>::insert(&user_account, new_vault_info);
			Ok(collateral_amount)
		}

		/// Withdraw collateral from a vault.
		///
		/// Parameters:
		/// - `user_account`: The account of the user that is withdrawing the collateral.
		/// - `collateral_amount`: The amount of collateral that is being withdrawn.
		pub fn do_withdraw_collateral(
			user_account: &T::AccountId,
			collateral_amount: BalanceOf<T>,
		) -> Result<BalanceOf<T>, DispatchError> {
			let vault_info = Self::vault_info(user_account)?;

			ensure!(vault_info.status.are_withdrawals_allowed(), <Error<T>>::WithdrawalsNotAllowed);

			let from = <Self as Vault>::account_id(&vault_info.id);

			let collateral_info = vault_info.collateral;

			ensure!(collateral_info.amount >= collateral_amount, <Error<T>>::NotEnoughCollateral);

			T::Currency::transfer(
				&from,
				user_account,
				collateral_amount,
				ExistenceRequirement::KeepAlive,
			)
				.map_err(|_| <Error<T>>::NotEnoughCollateralForFee)?;

			let deposit_time = <frame_system::Pallet<T>>::block_number();
			let new_collateral_info =
				CollateralInfo { amount: collateral_info.amount - collateral_amount, deposit_time };
			let new_vault_info = VaultInfo { collateral: new_collateral_info, ..vault_info };
			<Vaults<T>>::insert(&user_account, new_vault_info);
			Ok(collateral_amount)
		}

		/// Helper function to check if a given user is a registered matching_engine operator
		///
		/// Parameters:
		/// * `matching_engine_operator_account`: The matching_engine operator account that is being checked.
		pub fn is_registered_matching_engine_operator(matching_engine_operator_account: &T::AccountId) -> bool {
			<RegisteredMatchingEngine<T>>::contains_key(matching_engine_operator_account)
		}

		/// Helper function to check if a given user is registered.
		///
		/// Parameters:
		/// - `user_account`: The account of the user.
		pub fn is_registered_user(user_account: &T::AccountId) -> bool {
			<RegisteredUser<T>>::contains_key(user_account)
		}

		/// Helper function to check if a given account is registered as proxy.
		///
		/// Parameters:
		/// - `user_account`: The account of the user.
		/// - `proxy_account`: The account of the user.
		pub fn is_registered_proxy_account(
			user_account: &T::AccountId,
			proxy_account: T::AccountId,
		) -> bool {
			ProxyAccounts::<T>::get(user_account)
				.contains(&ProxyDefinition { proxy: proxy_account })
		}

		/// Unregister a Proxy Account for a given registered user.
		///
		/// Parameters:
		/// - `delegator`: The origin of the extrinsic. The user account that is unregistering the proxy account.
		/// - `proxy_account`: The proxy account that is being unregistered.
		#[require_transactional]
		pub fn remove_proxy_account(
			delegator: &T::AccountId,
			proxy_account: T::AccountId,
		) -> DispatchResult {
			// Verify that the delegator is a registered account.
			ensure!(Self::is_registered_user(delegator), <Error<T>>::NotARegisteredUserAccount);
			// Remove the account from the proxy account storage.
			ProxyAccounts::<T>::try_mutate_exists(delegator, |x| {
				let mut proxy_accounts = x.take().ok_or(<Error<T>>::NotRegisteredProxyAccounts)?;
				let proxy_definition = ProxyDefinition { proxy: proxy_account.clone() };
				let i = proxy_accounts
					.binary_search(&proxy_definition)
					.ok()
					.ok_or(<Error<T>>::NotARegisteredProxyAccount)?;
				proxy_accounts.remove(i);
				if !proxy_accounts.is_empty() {
					*x = Some(proxy_accounts)
				}
				Self::deposit_event(Event::ProxyAccountUnregistered(
					delegator.clone(),
					proxy_account,
				));
				Ok(())
			})
		}

		/// Transfer collateral from one vault to another.
		///
		/// Parameters:
		/// - `from_account`: The account of the user that is transferring the collateral.
		/// - `to_account`: The account of the user that is receiving the collateral.
		/// - `collateral_amount`: The amount of collateral that is being transferred.
		pub fn transfer_collateral(
			from_account: &T::AccountId,
			to_account: &T::AccountId,
			collateral_amount: BalanceOf<T>,
		) -> DispatchResult {
			let from_vault_info = Self::vault_info(from_account)?;
			let to_vault_info = Self::vault_info(to_account)?;

			ensure!(
				from_vault_info.status.is_active() && to_vault_info.status.is_active(),
				<Error<T>>::InactiveVault
			);

			let from = <Self as Vault>::account_id(&from_vault_info.id);
			let to = <Self as Vault>::account_id(&to_vault_info.id);

			let from_collateral_info = from_vault_info.collateral;
			let to_collateral_info = to_vault_info.collateral;

			ensure!(
				from_collateral_info.amount >= collateral_amount,
				<Error<T>>::NotEnoughCollateral
			);

			T::Currency::transfer(&from, &to, collateral_amount, ExistenceRequirement::KeepAlive)
				.map_err(|_| <Error<T>>::NotEnoughCollateralForFee)?;

			let deposit_time = <frame_system::Pallet<T>>::block_number();
			let new_from_collateral_info = CollateralInfo {
				amount: from_collateral_info.amount - collateral_amount,
				deposit_time,
			};
			let new_to_collateral_info = CollateralInfo {
				amount: to_collateral_info.amount + collateral_amount,
				deposit_time,
			};
			let new_from_vault_info =
				VaultInfo { collateral: new_from_collateral_info, ..from_vault_info };
			let new_to_vault_info =
				VaultInfo { collateral: new_to_collateral_info, ..to_vault_info };
			<Vaults<T>>::insert(&from_account, new_from_vault_info);
			<Vaults<T>>::insert(&to_account, new_to_vault_info);
			Ok(())
		}

		/// Helper function to fetch the vault info for a given user account.
		///
		/// Parameters:
		/// - `user_account`: The account of the user.
		fn vault_info(
			user_account: &T::AccountId,
		) -> Result<VaultInfo<T::AccountId, BalanceOf<T>, BlockNumberFor<T>, T::VaultId>, DispatchError>
		{
			Ok(<Vaults<T>>::try_get(user_account).map_err(|_err| <Error<T>>::VaultDoesNotExist)?)
		}

		/// Helper function to verify if the vault_owner possess sufficient amount to carry out the transaction.
		///
		/// Parameters:
		/// - `amount`: The account of the user.
		/// - `vault_owner`: AccountId of the vault owner.
		pub fn verify_collateral_amount(
			amount: u64,
			vault_owner: &T::AccountId,
		) -> bool {
			let vault_info = Self::vault_info(vault_owner).expect("VaultDoesNotExist");
			// Todo: Add a variable fee to calculate fee value for the transaction and check (amount + fee < collateral_amount)
			let fee= 1000u64;
			let change: BalanceOf<T> = amount.checked_add(fee).unwrap().saturated_into();
			vault_info.collateral.amount > change
		}

	}

	impl<T: Config> Vault for Pallet<T> {
		type AccountId = T::AccountId;
		type Balance = BalanceOf<T>;
		type BlockNumber = BlockNumberFor<T>;
		type VaultId = T::VaultId;

		fn account_id(vault_id: &Self::VaultId) -> Self::AccountId {
			sp_runtime::traits::AccountIdConversion::try_into_sub_account(
				&T::PalletId::get(), vault_id).unwrap()
		}

		fn create(account_id: Self::AccountId) -> Result<Self::VaultId, DispatchError> {
			Self::create_vault(account_id).map(|(vault_id, _)| vault_id)
		}

		fn deposit(
			from: &Self::AccountId,
			amount: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			// TODO: Add a minimum deposit amount.
			Self::do_deposit_collateral(from, amount)
		}

		fn withdraw(
			from: &Self::AccountId,
			amount: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			// TODO: Add a minimum withdrawal amount.
			Self::do_withdraw_collateral(from, amount)
		}
	}

	impl<T: Config> VaultWithStatus for Pallet<T> {
		fn allow_deposits(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					ensure!(!vault.status.is_closed(), <Error<T>>::DepositsNotAllowed);
					vault.status.allow_deposits();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn allow_withdrawals(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					vault.status.allow_withdrawals();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn are_deposits_allowed(account_id: &Self::AccountId) -> Result<bool, DispatchError> {
			Self::vault_info(account_id).map(|vault| vault.status.are_deposits_allowed())
		}

		fn are_withdrawals_allowed(account_id: &Self::AccountId) -> Result<bool, DispatchError> {
			Self::vault_info(account_id).map(|vault| vault.status.are_withdrawals_allowed())
		}

		fn close(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					ensure!(!vault.status.is_closed(), <Error<T>>::VaultClosed);
					vault.status.set_closed();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn freeze(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					ensure!(!vault.status.is_closed(), <Error<T>>::VaultClosed);
					vault.status.set_frozen();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn is_closed(account_id: &Self::AccountId) -> Result<bool, DispatchError> {
			Self::vault_info(account_id).map(|vault| vault.status.is_closed())
		}

		fn is_frozen(account_id: &Self::AccountId) -> Result<bool, DispatchError> {
			Self::vault_info(account_id).map(|vault| vault.status.is_frozen())
		}

		fn stop_deposits(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					vault.status.stop_deposits();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn stop_withdrawals(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					vault.status.stop_withdrawals();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn unclose(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					ensure!(vault.status.is_closed(), <Error<T>>::VaultNotClosed);
					vault.status.unclose();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}

		fn unfreeze(account_id: &Self::AccountId) -> DispatchResult {
			<Vaults<T>>::try_mutate_exists(account_id, |vault| {
				if let Some(vault) = vault {
					ensure!(!vault.status.is_closed(), <Error<T>>::VaultClosed);
					vault.status.unfreeze();
					Ok(())
				} else {
					Err(DispatchError::Other("Error in fetching vault."))
				}
			})
		}
	}
}
