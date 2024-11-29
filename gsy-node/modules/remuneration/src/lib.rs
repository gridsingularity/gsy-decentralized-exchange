// Copyright (C) SUPSI-DACD-ISAAC (www.supsi.ch/isaac)
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

//! # Remuneration
//!
//! The Remuneration module manages the administration and financial interactions in a decentralized energy exchange system.
//! It facilitates the tracking of energy communities, their prosumers, and their financial transactions, while ensuring that operations are governed by a designated custodian user.
//! This module is integral to maintaining accountability, ensuring transparent record-keeping, and simplifying energy trade settlements among participants.
//!
//! ## Features
//! - **Custodian Management**: A single, designated custodian user is granted super-user privileges, allowing them to oversee and manage all aspects of the module, including adding, removing, or updating energy communities and prosumers.
//! - **Energy Community Mapping**: Maintains a registry of energy communities and their respective managing entities, ensuring organized and scalable representation.
//! - **Prosumer Association**: Each prosumer is mapped to a single energy community, facilitating localized management of participants within the energy network.
//! - **Balance Tracking**: Tracks the balances of prosumers, ensuring accurate records of financial holdings to support trade and payment operations.
//! - **Payment Ledger**: Provides a detailed ledger for payments made between prosumers, including timestamps and metadata, enabling full transparency of financial transactions.


#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::weights::RemunerationWeightInfo;
pub use pallet::*;

// use frame_support::BoundedVec;
// use frame_support::pallet_prelude::*;
// use frame_system::pallet_prelude::*;

// #[cfg(test)]
// mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::RemunerationWeightInfo;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::UnixTime};
	use frame_support::{transactional};
	use frame_system::{ensure_signed, pallet_prelude::*};
	use sp_std::vec;

	/// # Remuneration Handler Trait
	///
	/// A trait that provides an interface for handling remuneration-related functionality.
	/// This allows other pallets to trigger remuneration actions, such as adding payments,
	/// without directly interacting with the underlying implementation of the remuneration pallet.
	///
	/// ## Methods
	/// - `add_payment(sender: AccountId, receiver: AccountId, amount: Balance) -> DispatchResult`:
	///   Handles the logic for adding a payment between two accounts. This method abstracts
	///   the internal workings of the remuneration pallet, enabling seamless integration with other pallets.
	pub trait RemunerationHandler<AccountId, Balance> {
		fn add_payment(sender: AccountId, receiver: AccountId, amount: Balance) -> DispatchResult;
	}

	/// # Configuration Trait for Remuneration Pallet
	///
	/// This trait defines the necessary configuration requirements for the remuneration pallet.
	/// It ensures that the pallet has access to required types and traits from other parts of the runtime.
	///
	/// ## Associated Types
	/// - `RuntimeEvent`: Represents the event type used within the pallet. It should map to the runtime's event type.
	/// - `RemunerationWeightInfo`: Provides weight information for benchmarking and transaction fees.
	/// - `RemunerationHandler`: A trait defining the handler for remuneration operations. This allows the remuneration
	///   logic to be triggered by other pallets in a modular and decoupled manner.
	#[pallet::config]
	pub trait Config:
	frame_system::Config + pallet_balances::Config + orderbook_registry::Config + orderbook_worker::Config
	{
		/// The overarching runtime event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the remuneration pallet's extrinsics.
		type RemunerationWeightInfo: RemunerationWeightInfo;

		/// A handler for remuneration operations, enabling interaction with the pallet's functionalities from other modules.
		type RemunerationHandler: RemunerationHandler<Self::AccountId, BalanceOf<Self>>;
	}

	/// # Implementation of the RemunerationHandler Trait for the Pallet
	///
	/// This implementation connects the `RemunerationHandler` trait to the remuneration pallet,
	/// enabling the use of the `add_payment` function. The implementation ensures that the logic
	/// defined in the remuneration pallet can be triggered by other modules in a consistent manner.
	///
	/// ## Methods
	/// - `add_payment(sender: T::AccountId, receiver: T::AccountId, amount: BalanceOf<T>) -> DispatchResult`:
	///   Invokes the `add_payment` method in the remuneration pallet using the root origin.
	impl<T: Config> RemunerationHandler<T::AccountId, BalanceOf<T>> for Pallet<T> {
		fn add_payment(sender: T::AccountId, receiver: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			Pallet::<T>::add_payment(
				frame_system::RawOrigin::Root.into(),
				sender,
				receiver,
				amount,
			)
		}
	}

	type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// # Storage Items

	/// ## Balances
	/// Tracks the financial balance of each participant in the energy market, including prosumers and community administrators.
	/// - Key: The account ID (`T::AccountId`) of the participant.
	/// - Value: The balance of the participant (`BalanceOf<T>`).
	/// - Access: `balances(account_id)` returns the balance of the given account.
	#[pallet::storage]
	#[pallet::getter(fn balances)]
	pub(super) type Balances<T: Config> = StorageMap<
		_,
		// Key: Account ID of the participant.
		Twox64Concat, T::AccountId,
		// Value: The participant's financial balance.
		BalanceOf<T>,
		ValueQuery
	>;

	/// ## PaymentDetails Struct
	/// Represents the details of a financial transaction between two participants.
	/// - `amount`: The amount of the payment.
	/// - `timestamp`: The timestamp of the payment (in seconds since the Unix epoch).
	/// - `metadata`: Optional metadata about the payment (e.g., description), limited to 256 bytes.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct PaymentDetails<Balance> {
		pub amount: Balance,                 // Payment amount
		pub timestamp: u32,                  // Timestamp of the payment
		pub metadata: Option<BoundedVec<u8, ConstU32<256>>>, // Bounded metadata, max 256 bytes
	}

	/// ## Payments Ledger
	/// Stores a record of payments between participants.
	/// - Key: A tuple `(sender_account_id, receiver_account_id, timestamp)`.
	/// - Value: `PaymentDetails`, which includes the payment amount, timestamp, and optional metadata.
	/// - Access: `payments(sender, receiver, timestamp)` retrieves the payment details for a given transaction.
	#[pallet::storage]
	#[pallet::getter(fn payments)]
	pub(super) type Payments<T: Config> = StorageNMap<
		_,
		(
			// Sender identifier (account ID).
			NMapKey<Blake2_128Concat, T::AccountId>,
			// Receiver identifier (account ID).
			NMapKey<Blake2_128Concat, T::AccountId>,
			// Timestamp of the payment.
			NMapKey<Blake2_128Concat, u32>,
		),
		// Details of the payment transaction.
		PaymentDetails<BalanceOf<T>>,
		OptionQuery,
	>;

	/// ## Communities
	/// Maintains a mapping of energy communities and their corresponding DSOs (Distribution System Operators).
	/// - Key: The community's identifier (`T::AccountId`).
	/// - Value: The account ID of the DSO responsible for managing the community.
	/// - Access: `communities(community_id)` returns the DSO account ID for a given community.
	#[pallet::storage]
	#[pallet::getter(fn communities)]
	pub(super) type Communities<T: Config> = StorageMap<
		_,
		// Community identifier.
		Twox64Concat, T::AccountId,
		// DSO account ID.
		T::AccountId,
		OptionQuery
	>;

	/// ## Prosumers
	/// Stores the association between prosumers and their respective energy communities.
	/// - Key: The account ID of the prosumer (`T::AccountId`).
	/// - Value: The identifier of the community to which the prosumer belongs.
	/// - Access: `prosumers(prosumer_id)` returns the community ID for a given prosumer.
	#[pallet::storage]
	#[pallet::getter(fn prosumers)]
	pub(super) type Prosumers<T: Config> = StorageMap<
		_,
		// Prosumer's account ID.
		Twox64Concat, T::AccountId,
		// Associated community's account ID.
		T::AccountId,
		OptionQuery
	>;

	/// ## Custodian
	/// Stores the account ID of the designated custodian user, who has super-user permissions within the module.
	/// - Value: The account ID of the custodian (`T::AccountId`).
	/// - Access: `custodian()` returns the account ID of the current custodian, if any.
	#[pallet::storage]
	#[pallet::getter(fn custodian)]
	pub(super) type Custodian<T: Config> = StorageValue<
		_,
		T::AccountId,
		OptionQuery
	>;

	/// # Events
	///
	/// The `Event` enum defines all the possible events that can be emitted by the Remuneration module.
	///
	/// ## Events
	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Emitted when the custodian user is updated.
		/// - `custodian`: The new custodian's account ID.
		CustodianUpdated { custodian: T::AccountId },

		/// Emitted when a new energy community is added.
		/// - `community`: The ID of the new community.
		/// - `dso`: The account ID of the DSO managing the community.
		CommunityAdded { community: T::AccountId, dso: T::AccountId },

		/// Emitted when an energy community is removed.
		/// - `community`: The ID of the community that was removed.
		CommunityRemoved { community: T::AccountId },

		/// Emitted when a new prosumer is added to a community.
		/// - `prosumer`: The account ID of the added prosumer.
		/// - `community`: The ID of the community to which the prosumer belongs.
		ProsumerAdded { prosumer: T::AccountId, community: T::AccountId },

		/// Emitted when a prosumer is removed from a community.
		/// - `prosumer`: The account ID of the removed prosumer.
		ProsumerRemoved { prosumer: T::AccountId },

		/// Emitted when a payment is added to the ledger.
		/// - `sender`: The account ID of the prosumer initiating the payment.
		/// - `receiver`: The account ID of the prosumer receiving the payment.
		/// - `amount`: The amount of the payment.
		/// - `timestamp`: The timestamp of the payment in seconds since the Unix epoch.
		PaymentAdded {
			sender: T::AccountId,
			receiver: T::AccountId,
			amount: BalanceOf<T>,
			timestamp: u32,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is not the custodian and cannot perform this action.
		NotCustodian,
		/// The sender and receiver cannot be the same.
		SameSenderReceiver,
		/// The sender does not have enough balance to complete the transaction.
		InsufficientBalance,
		/// The sender is not a registered prosumer.
		SenderNotProsumer,
		/// The receiver is not a registered prosumer.
		ReceiverNotProsumer,
	}

	/// # Dispatchable Calls for the Remuneration Pallet
	///
	/// This section defines all the callable functions (extrinsics) of the remuneration pallet.
	/// These functions allow users and other pallets to interact with the pallet's storage and logic,
	/// ensuring proper access control and event generation for key actions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// ## Update Custodian
		///
		/// Allows updating the custodian user. If no custodian is set, any user can initialize it.
		/// Otherwise, only the current custodian can perform this action.
		///
		/// - **Parameters**:
		///   - `new_custodian`: The account ID of the new custodian.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the current custodian (if one exists).
		///
		/// - **Event**:
		///   - `CustodianUpdated` is emitted upon success.
		#[transactional]
		#[pallet::weight(< T as Config >::RemunerationWeightInfo::update_custodian())]
		#[pallet::call_index(1)]
		pub fn update_custodian(origin: OriginFor<T>, new_custodian: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Check if a custodian is already defined
			match Custodian::<T>::get() {
				Some(current_custodian) => {
					// Ensure only the current custodian can update the custodian
					ensure!(sender == current_custodian, Error::<T>::NotCustodian);
				},
				None => {
					// If no custodian is defined, allow anyone to set it
				},
			}

			// Update the custodian
			Custodian::<T>::put(new_custodian.clone());
			Self::deposit_event(Event::CustodianUpdated { custodian: new_custodian });
			Ok(())
		}

		/// ## Add Community
		///
		/// Adds a new energy community to the system.
		///
		/// - **Parameters**:
		///   - `community`: The account ID of the community.
		///   - `dso`: The account ID of the DSO (Distribution System Operator) managing the community.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the custodian.
		///
		/// - **Event**:
		///   - `CommunityAdded` is emitted upon success.
		#[transactional]
		#[pallet::weight(< T as Config >::RemunerationWeightInfo::add_community())]
		#[pallet::call_index(2)]
		pub fn add_community(origin: OriginFor<T>, community: T::AccountId, dso: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure only the custodian can perform this action
			ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

			// Add the community to the map
			Communities::<T>::insert(community.clone(), dso.clone());
			Self::deposit_event(Event::CommunityAdded { community, dso });
			Ok(())
		}

		/// ## Remove Community
		///
		/// Removes an existing community from the system.
		///
		/// - **Parameters**:
		///   - `community`: The account ID of the community to remove.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the custodian.
		///
		/// - **Event**:
		///   - `CommunityRemoved` is emitted upon success.
		#[transactional]
		#[pallet::weight(< T as Config >::RemunerationWeightInfo::remove_community())]
		#[pallet::call_index(3)]
		pub fn remove_community(origin: OriginFor<T>, community: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure only the custodian can perform this action
			ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

			// Remove the community from the map
			Communities::<T>::remove(community.clone());
			Self::deposit_event(Event::CommunityRemoved { community });
			Ok(())
		}

		/// ## Add Prosumer
		///
		/// Adds a prosumer to a specified community.
		///
		/// - **Parameters**:
		///   - `prosumer`: The account ID of the prosumer.
		///   - `community`: The account ID of the community to which the prosumer belongs.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the custodian.
		///
		/// - **Event**:
		///   - `ProsumerAdded` is emitted upon success.
		#[transactional]
		#[pallet::weight(< T as Config >::RemunerationWeightInfo::add_prosumer())]
		#[pallet::call_index(4)]
		pub fn add_prosumer(origin: OriginFor<T>, prosumer: T::AccountId, community: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure only the custodian can perform this action
			ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

			// Add the prosumer to the map
			Prosumers::<T>::insert(prosumer.clone(), community.clone());
			Self::deposit_event(Event::ProsumerAdded { prosumer, community });
			Ok(())
		}
		/// ## Remove Prosumer
		///
		/// Removes a prosumer from the system.
		///
		/// - **Parameters**:
		///   - `prosumer`: The account ID of the prosumer to remove.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the custodian.
		///
		/// - **Event**:
		///   - `ProsumerRemoved` is emitted upon success.
		#[transactional]
		#[pallet::weight(< T as Config >::RemunerationWeightInfo::remove_prosumer())]
		#[pallet::call_index(5)]
		pub fn remove_prosumer(origin: OriginFor<T>, prosumer: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure only the custodian can perform this action
			ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

			// Remove the prosumer from the map
			Prosumers::<T>::remove(prosumer.clone());
			Self::deposit_event(Event::ProsumerRemoved { prosumer });
			Ok(())
		}

		/// ## Update Prosumer's Community
		///
		/// Updates the community association for a given prosumer.
		///
		/// - **Parameters**:
		///   - `prosumer`: The account ID of the prosumer.
		///   - `new_community`: The account ID of the new community.
		///
		/// - **Access Control**:
		///   - Requires the caller to be the custodian.
		///
		/// - **Event**:
		///   - `ProsumerAdded` is emitted with the new community.
		#[transactional]
		#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_prosumer())]
		#[pallet::call_index(6)]
		pub fn update_prosumer(
			origin: OriginFor<T>,
			prosumer: T::AccountId,
			new_community: T::AccountId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			// Ensure only the custodian can perform this action
			ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

			// Ensure the prosumer exists in the map
			ensure!(Prosumers::<T>::contains_key(&prosumer), Error::<T>::SenderNotProsumer);

			// Update the prosumer's community
			Prosumers::<T>::insert(prosumer.clone(), new_community.clone());
			Self::deposit_event(Event::ProsumerAdded {
				prosumer,
				community: new_community,
			});

			Ok(())
		}

		/// ## Add Payment
		///
		/// Records a payment transaction between two prosumers.
		///
		/// - **Parameters**:
		///   - `sender`: The account ID of the sender.
		///   - `receiver`: The account ID of the receiver.
		///   - `amount`: The payment amount.
		///
		/// - **Access Control**:
		///   - Requires the caller to be a valid signer.
		///
		/// - **Event**:
		///   - `PaymentAdded` is emitted with details of the transaction.
		///
		/// - **Validation**:
		///   - Ensures the sender and receiver are not the same.
		///   - Verifies both sender and receiver are registered in the system.
		///   - Ensures the sender has sufficient balance for the transaction.
		#[transactional]
		#[pallet::weight(<T as Config>::RemunerationWeightInfo::add_payment())]
		#[pallet::call_index(7)]
		pub fn add_payment(
			origin: OriginFor<T>,
			sender: T::AccountId,
			receiver: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			// Ensure the caller is authorized to perform the action
			let _who = ensure_signed(origin)?;

			// Ensure the sender and receiver are not the same
			ensure!(sender != receiver, Error::<T>::SameSenderReceiver);

			// Ensure the sender and receiver are in the Prosumers map
			ensure!(Prosumers::<T>::contains_key(&sender), Error::<T>::SenderNotProsumer);
			ensure!(Prosumers::<T>::contains_key(&receiver), Error::<T>::ReceiverNotProsumer);

			// Fetch the sender's balance and check its balance
			let sender_balance = Balances::<T>::get(&sender);
			ensure!(sender_balance >= amount, Error::<T>::InsufficientBalance);

			// Fetch the receiver's balance
			let receiver_balance = Balances::<T>::get(&receiver);
			// todo Here a filter to manage receiver balance overflows must be inserted

			let updated_sender_balance = sender_balance - amount;
			let updated_receiver_balance = receiver_balance + amount;
			// let updated_sender_balance = sender_balance.saturating_sub(amount);
			// let updated_receiver_balance = receiver_balance.saturating_add(amount);

			// Update balances
			Balances::<T>::insert(&sender, updated_sender_balance);
			Balances::<T>::insert(&receiver, updated_receiver_balance);

			let now = T::TimeProvider::now().as_secs() as u32;

			// Register the payment in the Payments map
			let payment_details = PaymentDetails {
				amount,
				timestamp: now,
				metadata: None,
			};
			Payments::<T>::insert((sender.clone(), receiver.clone(), now), payment_details);

			// Emit the PaymentAdded event
			Self::deposit_event(Event::PaymentAdded {
				sender,
				receiver,
				amount,
				timestamp: now,
			});

			Ok(())
		}
	}

	/// # Queries for the Remuneration Pallet
	///
	/// This implementation block provides utility functions for querying data from the pallet's storage.
	/// These functions enable external modules or users to access the stored information in a
	/// structured and efficient manner. The methods included here are read-only and do not alter the state
	/// of the storage.
	impl<T: Config> Pallet<T> {
		/// Query the balance of a specific account.
		///
		/// - **Parameters**:
		///   - `account_id`: The account ID of the participant.
		///
		/// - **Returns**:
		///   - The balance of the specified account.
		pub fn query_balance(account_id: T::AccountId) -> BalanceOf<T> {
			Self::balances(account_id)
		}

		/// Query the details of a specific payment.
		///
		/// - **Parameters**:
		///   - `sender`: The account ID of the sender.
		///   - `receiver`: The account ID of the receiver.
		///   - `timestamp`: The timestamp of the payment.
		///
		/// - **Returns**:
		///   - The `PaymentDetails` struct for the specified payment.
		pub fn query_payment(
			sender: T::AccountId,
			receiver: T::AccountId,
			timestamp: u32,
		) -> Option<PaymentDetails<BalanceOf<T>>> {
			Self::payments((sender, receiver, timestamp))
		}

		/// Query the DSO managing a specific community.
		///
		/// - **Parameters**:
		///   - `community_id`: The account ID of the community.
		///
		/// - **Returns**:
		///   - The account ID of the DSO managing the community, if exists.
		pub fn query_community_dso(community_id: T::AccountId) -> Option<T::AccountId> {
			Self::communities(community_id)
		}

		/// Query the community a specific prosumer belongs to.
		///
		/// - **Parameters**:
		///   - `prosumer_id`: The account ID of the prosumer.
		///
		/// - **Returns**:
		///   - The account ID of the community the prosumer belongs to, if exists.
		pub fn query_prosumer_community(prosumer_id: T::AccountId) -> Option<T::AccountId> {
			Self::prosumers(prosumer_id)
		}

		/// Query the current custodian user.
		///
		/// - **Returns**:
		///   - The account ID of the current custodian, if exists.
		pub fn query_custodian() -> Option<T::AccountId> {
			Self::custodian()
		}
	}
}
