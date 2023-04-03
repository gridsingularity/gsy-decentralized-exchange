// This file is part of GSy Decentralized Energy Exchange.

// Copyright 2022 Grid Singularity

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! # Orderbook Registry ( orderbook-registry )
//!
//!
//! The orderbook registry manages the orderbook of a the GSy-Decentralized Energy
//! Exchange. This module allows the registered user to add or delete an order
//! in the system. Moreover it enables a transparent verification of the orders references and
//! update their status after the order execution.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::Hash;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_support::{require_transactional, traits::Currency, transactional};
	use frame_system::pallet_prelude::*;
	use gsy_primitives::v0::{OrderReference, OrderStatus};
	use scale_info::TypeInfo;

	pub type BalanceOf<T> =
			<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
			TypeInfo
	)]
	pub struct ProxyDefinition<AccountId> {
		// The account which may act as proxy.
		pub proxy: AccountId,
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The Currency handler.
		type Currency: Currency<Self::AccountId>;

		/// The maximum number of proxy account a registered user can have.
		#[pallet::constant]
		type ProxyAccountLimit: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

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

	// Storage items.
	#[pallet::storage]
	#[pallet::getter(fn order_registry)]
	/// Keeps track of the orders for each registered user.
	pub type OrdersRegistry<T: Config> =
			StorageMap<_, Twox64Concat, OrderReference<T::AccountId, T::Hash>, OrderStatus, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Matching Engine operator has been registered. \[matching_engine_operator\]
		MatchingEngineOperatorRegistered(T::AccountId),
		/// New order has been inserted. \[depositor, hash\]
		NewOrderInserted(T::AccountId, T::Hash),
		/// New order has been inserted for the user by proxy. \[depositor, proxy, hash\]
		NewOrderInsertedByProxy(T::AccountId, T::AccountId, T::Hash),
		/// Order has been deleted. \[depositor, hash\]
		OrderDeleted(T::AccountId, T::Hash),
		/// Order has been deleted for the user by proxy. \[depositor, proxy, hash\]
		OrderDeletedByProxy(T::AccountId, T::AccountId, T::Hash),
		/// User has registered a proxy account. \[user_account, proxy_account\]
		ProxyAccountRegistered(T::AccountId, T::AccountId),
		/// User has unregistered a proxy account. \[user_account, proxy_account\]
		ProxyAccountUnregistered(T::AccountId, T::AccountId),
		/// User has been registered. \[user_account\]
		UserRegistered(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Cannot register an account already registered.
		AlreadyRegistered,
		/// Cannot register a proxy account already registered.
		AlreadyRegisteredProxyAccount,
		/// Cannot register a self proxy
		NoSelfProxy,
		/// Ensure that the account is a registered matching_engine operator.
		NotARegisteredMatchingEngineOperator,
		/// Ensure that the account is a proxy account.
		NotARegisteredProxyAccount,
		/// Ensure that the account is a registered user.
		NotARegisteredUserAccount,
		/// Ensure that the account is a proxy account.
		NotARegisteredUserOrProxyAccount,
		/// Ensure that the user has registered some proxy accounts.
		NotRegisteredProxyAccounts,
		/// Ensure the order exists.
		OpenOrderNotFound,
		/// Ensure the order exists and is not deleted.
		OrderAlreadyDeleted,
		/// Ensure the order exists and is not executed.
		OrderAlreadyExecuted,
		/// Ensure the order has not been already inserted.
		OrderAlreadyInserted,
		/// An account cannot have more proxy than `ProxyAccountLimit`.
		ProxyAccountsLimitReached,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Insert a new order.
		///
		/// Parameters
		/// `user_account`: The user who wants to insert the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::weight(29_000_000)]
		pub fn insert_orders(user_account: OriginFor<T>, order_hash: T::Hash) -> DispatchResult {
			let user_account = ensure_signed(user_account)?;
			// Verify that the user is a registered account.
			ensure!(Self::is_registered_user(&user_account), <Error<T>>::NotARegisteredUserAccount);
			let order_ref =
				OrderReference { user_id: user_account.clone(), hash: order_hash.clone() };
			let order_status = OrderStatus::Open;
			// Verify that the order is not already inserted.
			ensure!(!Self::is_order_registered(&order_ref), <Error<T>>::OrderAlreadyInserted);
			log::info!("inserting order: {:?} - status: {:?}", order_ref, order_status);
			<OrdersRegistry<T>>::insert(order_ref, order_status);
			Self::deposit_event(Event::NewOrderInserted(user_account, order_hash));
			Ok(())
		}

		/// Insert a new order with proxy account.
		///
		/// Parameters
		/// `proxy_account`: The user who wants to insert the order.
		/// `delegator`: The user who is delegating the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::weight(29_000_000)]
		pub fn insert_orders_by_proxy(
			delegator: OriginFor<T>,
			proxy_account: T::AccountId,
			order_hash: T::Hash,
		) -> DispatchResult {
			let delegator = ensure_signed(delegator)?;
			// Verify that the user is a registered proxy account.
			ensure!(
				Self::is_registered_proxy_account(&delegator, proxy_account.clone()),
				<Error<T>>::NotARegisteredUserOrProxyAccount
			);
			let order_ref = OrderReference { user_id: delegator.clone(), hash: order_hash.clone() };
			let order_status = OrderStatus::Open;
			// Verify that the order is not already inserted.
			ensure!(!Self::is_order_registered(&order_ref), <Error<T>>::OrderAlreadyInserted);
			log::info!("inserting order: {:?} - status: {:?}", order_ref, order_status);
			<OrdersRegistry<T>>::insert(order_ref, order_status);
			Self::deposit_event(Event::NewOrderInsertedByProxy(
				delegator,
				proxy_account,
				order_hash,
			));
			Ok(())
		}

		/// Delete an order.
		///
		/// Parameters
		/// `user_account`: The user who wants to remove the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::weight(29_000_000)]
		pub fn delete_order(user_account: OriginFor<T>, order_hash: T::Hash) -> DispatchResult {
			let user_account = ensure_signed(user_account)?;
			// Verify that the user is a registered account.
			ensure!(Self::is_registered_user(&user_account), <Error<T>>::NotARegisteredUserAccount);
			let order_ref =
				OrderReference { user_id: user_account.clone(), hash: order_hash.clone() };
			let updated_order_status = OrderStatus::Deleted;
			// Verify that the order is already inserted.
			ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);
			log::info!("deleting order: {:?} - status: {:?}", order_ref, updated_order_status);
			Self::update_order_status(order_ref.clone(), updated_order_status.clone())?;
			Self::deposit_event(Event::OrderDeleted(order_ref.user_id, order_ref.hash));
			Ok(())
		}

		/// Delete an order with proxy account.
		///
		/// Parameters
		/// `proxy_account`: The user who wants to remove the order.
		/// `delegator`: The user who is delegating the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::weight(29_000_000)]
		pub fn delete_order_by_proxy(
			delegator: OriginFor<T>,
			proxy_account: T::AccountId,
			order_hash: T::Hash,
		) -> DispatchResult {
			let delegator = ensure_signed(delegator)?;
			// Verify that the user is a registered proxy account.
			ensure!(
				Self::is_registered_proxy_account(&delegator,proxy_account.clone()),
				<Error<T>>::NotARegisteredUserOrProxyAccount
			);
			let order_ref = OrderReference { user_id: delegator.clone(), hash: order_hash.clone() };
			let updated_order_status = OrderStatus::Deleted;
			// Verify that the order is already inserted.
			ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);
			log::info!("deleting order: {:?} - status: {:?}", order_ref, updated_order_status);
			Self::update_order_status(order_ref.clone(), updated_order_status.clone())?;
			Self::deposit_event(Event::OrderDeleted(order_ref.user_id, order_ref.hash));
			Ok(())
		}

		/// Register a proxy account for a given registered user.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The user account that is registering the proxy account.
		/// * `proxy_account`: The proxy account that is being registered.
		#[transactional]
		#[pallet::weight(29_000_000)]
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
		#[pallet::weight(29_000_000)]
		pub fn register_matching_engine_operator(
			origin: OriginFor<T>,
			matching_engine_operator_account: T::AccountId,
		) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!(
					"Registering matching_engine operator account: {:?}",
					matching_engine_operator_account
			);
			Self::add_matching_engine_operator(matching_engine_operator_account)
		}

		/// Register a new user in the System. (Only the root user can register a new user)
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The root user.
		/// * `user_account`: The account of the new user.
		#[transactional]
		#[pallet::weight(29_000_000)]
		pub fn register_user(origin: OriginFor<T>, user_account: T::AccountId) -> DispatchResult {
			// Verify that the user is root.
			ensure_root(origin)?;
			log::info!("Registering user - {:?} ", user_account);
			Self::add_user(user_account.clone())?;
			Ok(())
		}

		/// Unregister a proxy account for a given registered user.
		///
		/// # Parameters:
		/// * `origin`: The origin of the extrinsic. The user account that is unregistering the proxy account.
		/// * `proxy_account`: The proxy account that is being unregistered.
		#[transactional]
		#[pallet::weight(29_000_000)]
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
	}

	impl<T: Config> Pallet<T> {
		/// Register a new matching_engine operator account in the System.
		///
		/// # Parameters:
		/// * `matching_engine_operator_account`: The matching_engine operator account that is being registered.
		pub fn add_matching_engine_operator(
				matching_engine_operator_account: T::AccountId
		) -> DispatchResult {
			ensure!(
				!Self::is_registered_matching_engine_operator(&matching_engine_operator_account),
				<Error<T>>::AlreadyRegistered
			);
			let account_hash = T::Hashing::hash_of(&matching_engine_operator_account);
			log::info!("Account Hash - {:?} ", account_hash);
			<RegisteredMatchingEngine<T>>::insert(&matching_engine_operator_account, account_hash);
			// Deposit the MatchingEngineOperatorRegistered event.
			Self::deposit_event(Event::MatchingEngineOperatorRegistered(
					matching_engine_operator_account
			));
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

		/// Register a new user.
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


		/// Helper function to check if a given order has already been inserted.
		///
		/// Parameters
		/// `order_ref`: The order reference.
		pub fn is_order_registered(order_ref: &OrderReference<T::AccountId, T::Hash>) -> bool {
			<OrdersRegistry<T>>::contains_key(order_ref)
		}

		/// Helper function to check if a given user is a registered matching_engine operator
		///
		/// Parameters:
		/// * `matching_engine_operator_account`: The matching_engine operator account that is being checked.
		pub fn is_registered_matching_engine_operator(
				matching_engine_operator_account: &T::AccountId
		) -> bool {
			<RegisteredMatchingEngine<T>>::contains_key(matching_engine_operator_account)
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

		/// Helper function to check if a given user is registered.
		///
		/// Parameters:
		/// - `user_account`: The account of the user.
		pub fn is_registered_user(user_account: &T::AccountId) -> bool {
			<RegisteredUser<T>>::contains_key(user_account)
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

		/// Helper function to update the order status in the OrderRegistry.
		///
		/// Parameters
		/// `order_ref`: The order reference.
		/// `order_status`: The new order status.
		pub fn update_order_status(
			order_ref: OrderReference<T::AccountId, T::Hash>,
			updated_order_status: OrderStatus,
		) -> DispatchResult {
			// Verify that the bid and offer have already been inserted.
			ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);

			<OrdersRegistry<T>>::try_mutate(order_ref, |order_status| {
				if let OrderStatus::Open = order_status {
					*order_status = updated_order_status.clone();
					Ok(())
				} else if let OrderStatus::Executed = order_status {
					Err(<Error<T>>::OrderAlreadyExecuted)?
				} else if let OrderStatus::Deleted = order_status {
					Err(<Error<T>>::OrderAlreadyDeleted)?
				} else {
					Err(<Error<T>>::OpenOrderNotFound)?
				}
			})
		}
	}
}
