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
//! The orderbook registry manages the orderbook of the GSy-Decentralized Energy
//! Exchange. This module allows the registered user to add or delete an order
//! in the system. Moreover, it enables a transparent verification of the orders references and
//! update their status after the order execution.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	// Import various useful types required by all FRAME pallets.
	use super::*;
	use frame_support::dispatch::DispatchResult;
	use frame_support::{
		pallet_prelude::*, require_transactional, traits::Currency, traits::UnixTime, transactional,
	};
	use frame_system::pallet_prelude::*;
	use gsy_primitives::v0::{BidOfferMatch, OrderReference, OrderStatus, Trade, TradeParameters};
	use scale_info::{prelude::vec::Vec, TypeInfo};
	use sp_runtime::traits::Hash;
	use sp_runtime::SaturatedConversion;

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
		TypeInfo,
	)]
	pub struct ProxyDefinition<AccountId> {
		// The account which may act as proxy.
		pub proxy: AccountId,
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + gsy_collateral::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The Currency handler.
		type Currency: Currency<Self::AccountId>;

		/// The maximum number of proxy account a registered user can have.
		#[pallet::constant]
		type RegistryProxyAccountLimit: Get<u32>;
		type TimeProvider: UnixTime;
		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;
	}

	// Storage items.
	#[pallet::storage]
	#[pallet::getter(fn order_registry)]
	/// Keeps track of the orders for each registered user.
	pub type OrdersRegistry<T: Config> = StorageMap<
		_,
		Twox64Concat,
		OrderReference<T::AccountId, T::Hash>,
		OrderStatus<T::Hash>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn market_status)]
	/// Maps a unique market ID (deterministic hash of type + delivery_time) to its status.
	/// (true = Open, false = Closed).
	/// Defaults to `false` (Closed) via ValueQuery.
	pub type MarketStatus<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn executed_trades)]
	/// Keeps track of the executed trades by each registered matching_engine operator.
	pub type TradesRegistry<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, T::Hash, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New order has been inserted. \[depositor, hash\]
		NewOrderInserted(T::AccountId, T::Hash),
		/// New order has been inserted for the user by proxy. \[depositor, proxy, hash\]
		NewOrderInsertedByProxy(T::AccountId, T::AccountId, T::Hash),
		/// Notify that all orders in the batch have been inserted
		AllOrdersInserted(T::AccountId),
		/// Order has been deleted. \[depositor, hash\]
		OrderDeleted(T::AccountId, T::Hash),
		/// Order has been deleted for the user by proxy. \[depositor, proxy, hash\]
		OrderDeletedByProxy(T::AccountId, T::AccountId, T::Hash),
		/// Notify that all orders in the batch have been deleted
		AllOrdersDeleted(T::AccountId),
		/// Order has been executed. \[depositor, hash, selected_energy, energy_rate, time_slot\]
		OrderExecuted(Trade<T::AccountId, T::Hash>),
		/// Trade has been cleared.
		TradeCleared(T::Hash),
		/// A market's status has been updated on-chain. [market_uid, is_open]
		MarketStatusUpdated(T::Hash, bool),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Ensure the order exists.
		OpenOrderNotFound,
		/// Ensure the order exists and is not deleted.
		OrderAlreadyDeleted,
		/// Ensure the order exists and is not executed.
		OrderAlreadyExecuted,
		/// Ensure the order has not been already inserted.
		OrderAlreadyInserted,
		/// Ensure the transfer has been successful.
		UnableToCompleteTransfer,
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
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::orderbook_registry_weight())]
		pub fn insert_orders(
			user_account: OriginFor<T>,
			orders_hash: Vec<T::Hash>,
		) -> DispatchResult {
			let user_account = ensure_signed(user_account).unwrap();
			// Verify that the user is a registered account.
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_user(&user_account),
				gsy_collateral::Error::<T>::NotARegisteredUserAccount
			);
			for order_hash in orders_hash {
				let order_ref =
					OrderReference { user_id: user_account.clone(), hash: order_hash.clone() };
				let order_status = OrderStatus::Open;
				// Verify that the order is not already inserted.
				ensure!(!Self::is_order_registered(&order_ref), <Error<T>>::OrderAlreadyInserted);
				log::info!("inserting order: {:?} - status: {:?}", order_ref, order_status);
				<OrdersRegistry<T>>::insert(order_ref, order_status);
				Self::deposit_event(Event::NewOrderInserted(user_account.clone(), order_hash));
			}
			Self::deposit_event(Event::AllOrdersInserted(user_account.clone()));
			Ok(())
		}

		/// Insert a new order with proxy account.
		///
		/// Parameters
		/// `proxy_account`: The user who wants to insert the order.
		/// `delegator`: The user who is delegating the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::orderbook_registry_weight())]
		pub fn insert_orders_by_proxy(
			proxy_account: OriginFor<T>,
			delegator: T::AccountId,
			orders_hash: Vec<T::Hash>,
		) -> DispatchResult {
			let proxy_account = ensure_signed(proxy_account).unwrap();
			// Verify that the user is a registered proxy account.
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_proxy_account(
					&delegator,
					proxy_account.clone()
				),
				gsy_collateral::Error::<T>::NotARegisteredProxyAccount
			);
			for order_hash in orders_hash {
				let order_ref =
					OrderReference { user_id: delegator.clone(), hash: order_hash.clone() };
				let order_status = OrderStatus::Open;
				// Verify that the order is not already inserted.
				ensure!(!Self::is_order_registered(&order_ref), <Error<T>>::OrderAlreadyInserted);
				log::info!("inserting order: {:?} - status: {:?}", order_ref, order_status);
				<OrdersRegistry<T>>::insert(order_ref, order_status);
				Self::deposit_event(Event::NewOrderInsertedByProxy(
					delegator.clone(),
					proxy_account.clone(),
					order_hash,
				));
			}
			Ok(())
		}

		/// Delete an order.
		///
		/// Parameters
		/// `user_account`: The user who wants to remove the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::orderbook_registry_weight())]
		pub fn delete_orders(
			user_account: OriginFor<T>,
			orders_hash: Vec<T::Hash>,
		) -> DispatchResult {
			let user_account = ensure_signed(user_account).unwrap();
			// Verify that the user is a registered account.
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_user(&user_account),
				gsy_collateral::Error::<T>::NotARegisteredUserAccount
			);
			for order_hash in orders_hash {
				let order_ref =
					OrderReference { user_id: user_account.clone(), hash: order_hash.clone() };
				let updated_order_status = OrderStatus::Deleted;
				// Verify that the order is already inserted.
				ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);
				log::info!("deleting order: {:?} - status: {:?}", order_ref, updated_order_status);
				Self::update_order_status(order_ref.clone(), updated_order_status.clone())?;
				Self::deposit_event(Event::OrderDeleted(order_ref.user_id, order_ref.hash));
			}
			Self::deposit_event(Event::AllOrdersDeleted(user_account));
			Ok(())
		}

		/// Delete an order with proxy account.
		///
		/// Parameters
		/// `proxy_account`: The user who wants to remove the order.
		/// `delegator`: The user who is delegating the order.
		/// `order_hash`: The hash of the order.
		#[transactional]
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::orderbook_registry_weight())]
		pub fn delete_orders_by_proxy(
			proxy_account: OriginFor<T>,
			delegator: T::AccountId,
			orders_hash: Vec<T::Hash>,
		) -> DispatchResult {
			let proxy_account = ensure_signed(proxy_account).unwrap();
			// Verify that the user is a registered proxy account.
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_proxy_account(
					&delegator,
					proxy_account.clone()
				),
				gsy_collateral::Error::<T>::NotARegisteredProxyAccount
			);
			for order_hash in orders_hash {
				let order_ref =
					OrderReference { user_id: delegator.clone(), hash: order_hash.clone() };
				let updated_order_status = OrderStatus::Deleted;
				// Verify that the order is already inserted.
				ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);
				log::info!("deleting order: {:?} - status: {:?}", order_ref, updated_order_status);
				Self::update_order_status(order_ref.clone(), updated_order_status.clone())?;
				Self::deposit_event(Event::OrderDeleted(order_ref.user_id, order_ref.hash));
			}
			Ok(())
		}

		/// Update the on-chain status of a specific market (open or close it).
		///
		/// This is a privileged extrinsic that can only be called by a registered
		/// matching engine operator account. The new Market Orchestrator service must
		/// use the key of this account to successfully submit this transaction.
		///
		/// Parameters:
		/// - `origin`: The privileged account (Market Orchestrator).
		/// - `market_uid`: The deterministic hash (market_type + delivery_time) of the market.
		/// - `is_open`: The new status to set (true for Open, false for Closed).
		#[transactional]
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::orderbook_registry_weight())]
		pub fn update_market_status(
			origin: OriginFor<T>,
			market_uid: T::Hash,
			is_open: bool,
		) -> DispatchResult {
			let operator = ensure_signed(origin)?;
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_matching_engine_operator(&operator),
				gsy_collateral::Error::<T>::NotARegisteredMatchingEngineOperator
			);
			MarketStatus::<T>::insert(market_uid, is_open);
			Self::deposit_event(Event::MarketStatusUpdated(market_uid, is_open));
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Helper function to check if a given order has already been inserted.
		///
		/// Parameters
		/// `order_ref`: The order reference.
		pub fn is_order_registered(order_ref: &OrderReference<T::AccountId, T::Hash>) -> bool {
			<OrdersRegistry<T>>::contains_key(order_ref)
		}

		/// Helper function to update the order status in the OrderRegistry.
		///
		/// Parameters
		/// `order_ref`: The order reference.
		/// `order_status`: The new order status.
		pub fn update_order_status(
			order_ref: OrderReference<T::AccountId, T::Hash>,
			updated_order_status: OrderStatus<T::Hash>,
		) -> DispatchResult {
			// Verify that the bid and offer have already been inserted.
			ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);

			<OrdersRegistry<T>>::try_mutate(order_ref, |order_status| {
				if let OrderStatus::Open = order_status {
					*order_status = updated_order_status.clone();
					Ok(())
				} else if let OrderStatus::Executed(_) = order_status {
					Err(<Error<T>>::OrderAlreadyExecuted)?
				} else if let OrderStatus::Deleted = order_status {
					Err(<Error<T>>::OrderAlreadyDeleted)?
				} else {
					Err(<Error<T>>::OpenOrderNotFound)?
				}
			})
		}

		/// Execute a batch of orders.
		///
		/// Parameters
		/// `matching_engine_account`: The user who proposes tha trade match to execute the order.
		/// `proposed_matches`: The proposed match batch.
		#[require_transactional]
		pub fn clear_orders_batch(
			matching_engine_account: T::AccountId,
			proposed_matches: Vec<BidOfferMatch<T::AccountId>>,
		) -> DispatchResult {
			for proposed_match in proposed_matches {
				Self::clear_order(matching_engine_account.clone(), proposed_match)?;
			}
			Ok(())
		}

		/// Execute an order.
		///
		/// Parameters
		/// `matching_engine_account`: The user who proposes tha trade match to execute the order.
		/// `proposed_match`: The proposed match structure containing the bid and offer.
		#[require_transactional]
		pub fn clear_order(
			matching_engine_account: T::AccountId,
			proposed_match: BidOfferMatch<T::AccountId>,
		) -> DispatchResult {
			// Verify that the user is a registered matching_engine operator account.
			ensure!(
				<gsy_collateral::Pallet<T>>::is_registered_matching_engine_operator(
					&matching_engine_account
				),
				gsy_collateral::Error::<T>::NotARegisteredMatchingEngineOperator
			);

			let bid_hash = T::Hashing::hash_of(&proposed_match.bid);
			let offer_hash = T::Hashing::hash_of(&proposed_match.offer);
			let bid_ref =
				OrderReference { user_id: proposed_match.bid.buyer.clone(), hash: bid_hash };

			let offer_ref =
				OrderReference { user_id: proposed_match.offer.seller.clone(), hash: offer_hash };

			let mut orders_ref: Vec<OrderReference<T::AccountId, T::Hash>> = Vec::new();
			orders_ref.push(bid_ref.clone());
			orders_ref.push(offer_ref.clone());

			let trade_parameters = TradeParameters {
				selected_energy: proposed_match.selected_energy,
				energy_rate: proposed_match.energy_rate,
				trade_uuid: T::Hashing::hash_of(&proposed_match),
			};

			let trade = Trade {
				seller: proposed_match.offer.seller.clone(),
				buyer: proposed_match.bid.buyer.clone(),
				market_id: proposed_match.market_id,
				time_slot: proposed_match.time_slot,
				trade_uuid: T::Hashing::hash_of(&proposed_match),
				creation_time: T::TimeProvider::now().as_secs(),
				offer: proposed_match.offer.clone(),
				offer_hash: offer_ref.hash.clone(),
				bid: proposed_match.bid.clone(),
				bid_hash: bid_ref.hash.clone(),
				residual_offer: proposed_match.residual_offer.clone(),
				residual_bid: proposed_match.residual_bid.clone(),
				parameters: trade_parameters,
			};

			let updated_order_status = OrderStatus::Executed(trade_parameters.clone());

			log::info!(
				"executing trade with bid and offer: {:?} - status: {:?}",
				orders_ref,
				updated_order_status
			);

			for order_ref in orders_ref {
				ensure!(Self::is_order_registered(&order_ref), <Error<T>>::OpenOrderNotFound);
				Self::update_order_status(order_ref, updated_order_status.clone())?;
			}

			if proposed_match.bid.buyer.clone() != proposed_match.offer.seller.clone() {
				// Settle the trade with amount transferred from buyer to seller.

				<gsy_collateral::Pallet<T>>::transfer_collateral(
					&proposed_match.bid.buyer,
					&proposed_match.offer.seller,
					(proposed_match
						.selected_energy
						.checked_mul(proposed_match.energy_rate)
						.ok_or(<Error<T>>::OrderAlreadyInserted)?)
					.saturated_into(),
				)?;
			}

			// Add trade in the trade registry.
			<TradesRegistry<T>>::insert(
				matching_engine_account,
				T::Hashing::hash_of(&proposed_match),
			);

			Self::deposit_event(Event::TradeCleared(T::Hashing::hash_of(&proposed_match)));

			Self::deposit_event(Event::OrderExecuted(trade));

			Ok(())
		}

		/// Helper function to check if a given order has already been inserted.
		///
		/// Parameters
		/// `amount`: The order reference.
		/// `vault_owner`: AccountId of the vault owner.
		pub fn is_collateral_amount_sufficient(amount: u64, vault_owner: &T::AccountId) -> bool {
			<gsy_collateral::Pallet<T>>::verify_collateral_amount(amount, vault_owner)
		}
	}
}
