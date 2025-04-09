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
	pub use pallet::Error;
	pub use pallet::CommunityInfo;
	pub use pallet::{INTRA_COMMUNITY, INTER_COMMUNITY};
	

	#[cfg(test)]
	mod mock;

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

		pub const INTRA_COMMUNITY: u8 = 1;
		pub const INTER_COMMUNITY: u8 = 2;

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
			fn add_payment(receiver: AccountId, amount: Balance, payment_type: u8) -> DispatchResult;
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
		frame_system::Config + pallet_balances::Config + orderbook_registry::Config + scale_info::TypeInfo
		{
			/// The overarching runtime event type.
			type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

			/// Weight information for the remuneration pallet's extrinsics.
			type RemunerationWeightInfo: RemunerationWeightInfo;

			/// The length of the market slot in seconds.
			#[pallet::constant]
			type MarketSlotDuration: Get<u64>;

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
			fn add_payment(receiver: T::AccountId, amount: BalanceOf<T>, payment_type: u8) -> DispatchResult {
				Pallet::<T>::add_payment(
					frame_system::RawOrigin::Root.into(),
					receiver,
					amount,
					payment_type,
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
		#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
		pub struct CommunityInfo<T: Config> {
			pub dso: T::AccountId,
			pub owner: T::AccountId,
		}
		#[pallet::storage]
		#[pallet::getter(fn communities)]
		pub(super) type Communities<T: Config> = StorageMap<
			_,
			// Community identifier.
			Twox64Concat, T::AccountId,
			// New struct storing both DSO and Owner.
			CommunityInfo<T>,
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

		#[pallet::storage]
		#[pallet::getter(fn custodian_gsy)]
		pub(super) type CustodianGsy<T: Config> = StorageValue<
			_,
			gsy_primitives::v0::AccountId,
			OptionQuery
		>;

		/// ## Alpha Parameter
		/// Used in settlement calculations for under-delivery penalty.
		/// This is a fixed-point representation where 1.0 = 1_000_000
		#[pallet::storage]
		#[pallet::getter(fn alpha)]
		pub(super) type Alpha<T: Config> = StorageValue<
			_,
			u64,
			ValueQuery
		>;	
		
		/// ## Beta Parameter
		/// Used in settlement calculations for over-delivery adjustment.
		/// This is a fixed-point representation where 1.0 = 1_000_000
		#[pallet::storage]
		#[pallet::getter(fn beta)]
		pub(super) type Beta<T: Config> = StorageValue<
			_,
			u64,
			ValueQuery
		>;

		/// ## Tolerance Parameter
		/// Used in settlement calculations for acceptable deviation thresholds.
		/// This is a fixed-point representation where 1.0 = 1_000_000
		#[pallet::storage]
		#[pallet::getter(fn tolerance)]
		pub(super) type Tolerance<T: Config> = StorageValue<
			_,
			u64,
			ValueQuery
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
			CommunityAdded { community: T::AccountId, dso: T::AccountId, owner: T::AccountId },

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
				payment_type: u8,
				timestamp: u32,
			},

			/// Emitted when the custodian sets a user's balance in the Remuneration pallet.
			///
			/// - `user`: The account ID of the user whose local balance is updated.
			/// - `new_balance`: The new balance assigned to that user in this pallet's storage.
			BalanceSet {
				user: T::AccountId,
				new_balance: BalanceOf<T>,
			},

			/// Emitted when the Alpha parameter is updated.
			/// - `old_value`: The previous value.
			/// - `new_value`: The new value.
			AlphaUpdated { old_value: u64, new_value: u64 },

			/// Emitted when the Beta parameter is updated.
			/// - `old_value`: The previous value.
			/// - `new_value`: The new value.
			BetaUpdated { old_value: u64, new_value: u64 },

			/// Emitted when the Tolerance parameter is updated.
			/// - `old_value`: The previous value.
			/// - `new_value`: The new value.
			ToleranceUpdated { old_value: u64, new_value: u64 },

			/// Emitted when a flexibility settlement is processed.
			/// - `requester`: The account ID of the flexibility requester.
			/// - `provider`: The account ID of the flexibility provider.
			/// - `requested`: The requested flexibility amount.
			/// - `delivered`: The actually delivered flexibility amount.
			/// - `price`: The agreed price.
			/// - `calculated_amount`: The final calculated payment amount.
			FlexibilitySettled {
				requester: T::AccountId,
				provider: T::AccountId,
				requested: u64,
				delivered: u64,
				price: u64,
				calculated_amount: BalanceOf<T>,
			},			
		}

		#[pallet::error]
		pub enum Error<T> {
			/// The caller is not the custodian and cannot perform this action.
			NotCustodian,
			/// The caller is not allowed to manage (add/remove/update) prosumers (i.e. the caller is neither the custodian nor the community owner).
			NotAllowedToManageProsumers,
			/// The sender and receiver cannot be the same.
			SameSenderReceiver,
			/// The sender does not have enough balance to complete the transaction.
			InsufficientBalance,
			/// The sender is not a registered prosumer.
			SenderNotProsumer,
			/// The receiver is not a registered prosumer.
			ReceiverNotProsumer,
			/// Sender and receiver must belong to the same community (intra-community case).
			DifferentCommunities,
			/// Sender or receiver is not a registered community (inter-community case).
			NotACommunity,
			/// The caller is not the community owner.
			NotCommunityOwner,
			/// Payment not allowed (availbale only intra and inter community cases).
			PaymentTypeNotAllowed,
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
			pub fn add_community(origin: OriginFor<T>, community: T::AccountId, dso: T::AccountId, owner: T::AccountId) -> DispatchResult {
				let sender = ensure_signed(origin)?;

				// Ensure only the custodian can perform this action
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

				// Add the community to the map
				let community_info = CommunityInfo { dso, owner };
				Communities::<T>::insert(community.clone(), community_info.clone());
				Self::deposit_event(Event::CommunityAdded { community, dso: community_info.dso, owner: community_info.owner });
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

				// Remove the community from storage
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

				// Fetch the community info to check owner
				let community_info = Communities::<T>::get(&community).ok_or(Error::<T>::NotACommunity)?;

				// Ensure only the custodian or community owner can perform this action
				ensure!(
					Some(sender.clone()) == Custodian::<T>::get() || Some(sender.clone()) == Some(community_info.owner),
					Error::<T>::NotAllowedToManageProsumers
				);

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

				// Fetch the community the prosumer belongs to
				let community = Prosumers::<T>::get(&prosumer).ok_or(Error::<T>::SenderNotProsumer)?;

				// Fetch the community info (including the owner)
				let community_info = Communities::<T>::get(&community).ok_or(Error::<T>::NotACommunity)?;

				// Ensure only the custodian or community owner can perform this action
				ensure!(
					Some(sender.clone()) == Custodian::<T>::get() || Some(sender.clone()) == Some(community_info.owner),
					Error::<T>::NotAllowedToManageProsumers
				);

				// Remove the prosumer from the map
				Prosumers::<T>::remove(prosumer.clone());

				// Emit the event
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
				receiver: T::AccountId,
				amount: BalanceOf<T>,
				payment_type: u8
			) -> DispatchResult {
				// Ensure the caller is authorized to perform the action
				let sender = ensure_signed(origin)?;

				// Ensure the sender and receiver are not the same
				ensure!(sender != receiver, Error::<T>::SameSenderReceiver);

				if payment_type == INTRA_COMMUNITY {
					// Ensure the sender and receiver are in the prosumers map
					ensure!(Prosumers::<T>::contains_key(&sender), Error::<T>::SenderNotProsumer);
					ensure!(Prosumers::<T>::contains_key(&receiver), Error::<T>::ReceiverNotProsumer);

					// Ensure the sender and receiver belong to the same community
					let sender_community = Prosumers::<T>::get(&sender).ok_or(Error::<T>::SenderNotProsumer)?;
					let receiver_community = Prosumers::<T>::get(&receiver).ok_or(Error::<T>::ReceiverNotProsumer)?;
					ensure!(sender_community == receiver_community, Error::<T>::DifferentCommunities);
				}
				else if payment_type == INTER_COMMUNITY {
					// Ensure the sender and receiver are communities
					ensure!(Communities::<T>::contains_key(&sender), Error::<T>::NotACommunity);
					ensure!(Communities::<T>::contains_key(&receiver), Error::<T>::NotACommunity);
				}
				else {
					return Err(Error::<T>::PaymentTypeNotAllowed.into());
				}

				// Fetch the sender's balance and check its balance
				let sender_balance = Balances::<T>::get(&sender);
				if sender_balance < amount {
					return Err(Error::<T>::InsufficientBalance.into());
				}

				// Fetch the receiver's balance
				let receiver_balance = Balances::<T>::get(&receiver);

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
					payment_type: payment_type,
					timestamp: now,
				});

				Ok(())
			}

			/// ## Set Balance
			///
			/// Allows the *custodian* to directly set the Remuneration pallet's local
			/// balance for any specified user.
			///
			/// - **Parameters**:
			///   - `user`: The account ID of the user whose balance is being updated.
			///   - `new_balance`: The new balance to store in this pallet's local storage.
			///
			/// - **Access Control**:
			///   - Must be called by the custodian, as defined in `Custodian<T>`.
			///
			/// - **Event**:
			///   - `BalanceSet` is emitted with the user's account ID and the new balance.
			///
			/// - **Validation**:
			///   - This extrinsic does not check the chain's real balance (in `pallet_balances`).
			///   - It only updates the local `Balances<T>` map in this pallet.
			///
			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::set_balance())]
			#[pallet::call_index(8)]
			pub fn set_balance(
				origin: OriginFor<T>,
				user: T::AccountId,
				new_balance: BalanceOf<T>,
			) -> DispatchResult {
				// Make sure the caller is a signed origin
				let sender = ensure_signed(origin)?;

				// Only the custodian can perform this action
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

				// Update the local Balances storage map
				Balances::<T>::insert(&user, new_balance);

				Self::deposit_event(Event::BalanceSet { user, new_balance, });

				Ok(())
			}

			/// ## Update Alpha Parameter
			///
			/// Updates the alpha parameter used for under-delivery penalty calculation.
			///
			/// - **Parameters**:
			///   - `new_alpha`: The new alpha value (fixed-point, 1.0 = 1_000_000).
			///
			/// - **Access Control**:
			///   - Requires the caller to be the custodian.
			///
			/// - **Event**:
			///   - `AlphaUpdated` is emitted upon success.
			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_alpha())]
			#[pallet::call_index(13)]
			pub fn update_alpha(
				origin: OriginFor<T>,
				new_alpha: u64,
			) -> DispatchResult {
				// Make sure the caller is a signed origin
				let sender = ensure_signed(origin)?;

				// Only the custodian can perform this action
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

				// Get old value for event
				let old_alpha = Alpha::<T>::get();

				// Update the alpha parameter
				Alpha::<T>::put(new_alpha);

				// Emit the event
				Self::deposit_event(Event::AlphaUpdated { 
					old_value: old_alpha, 
					new_value: new_alpha 
				});

				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_beta())]
			#[pallet::call_index(14)]
			pub fn update_beta(
				origin: OriginFor<T>,
				new_beta: u64,
			) -> DispatchResult {
				// Make sure the caller is a signed origin
				let sender = ensure_signed(origin)?;

				// Only the custodian can perform this action
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

				// Get old value for event
				let old_beta = Beta::<T>::get();

				// Update the beta parameter
				Beta::<T>::put(new_beta);

				// Emit the event
				Self::deposit_event(Event::BetaUpdated { 
					old_value: old_beta, 
					new_value: new_beta 
				});

				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_tolerance())]
			#[pallet::call_index(15)]
			pub fn update_tolerance(
				origin: OriginFor<T>,
				new_tolerance: u64,
			) -> DispatchResult {
				// Make sure the caller is a signed origin
				let sender = ensure_signed(origin)?;

				// Only the custodian can perform this action
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);

				// Get old value for event
				let old_tolerance = Tolerance::<T>::get();

				// Update the tolerance parameter
				Tolerance::<T>::put(new_tolerance);

				// Emit the event
				Self::deposit_event(Event::ToleranceUpdated { 
					old_value: old_tolerance, 
					new_value: new_tolerance 
				});

				Ok(())
			}

			/// ## Settle Flexibility Payment
			///
			/// Calculates and processes a payment for flexibility services based on various parameters.
			///
			/// - **Parameters**:
			///   - `receiver`: The account ID of the flexibility provider.
			///   - `flexi_requested`: The requested flexibility amount.
			///   - `flexi_delivered`: The actually delivered flexibility amount.
			///   - `price`: The agreed price per unit.
			///   - `payment_type`: The type of payment (intra or inter community).
			///
			/// - **Access Control**:
			///   - Requires the caller to be a valid signer.
			///
			/// - **Events**:
			///   - `FlexibilitySettled` is emitted with details of the settlement.
			///   - `PaymentAdded` is emitted for the underlying payment.
			///
			/// - **Validation**:
			///   - Ensures all validation from the underlying `add_payment` function.
			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::settle_flexibility_payment())]
			#[pallet::call_index(16)]
			pub fn settle_flexibility_payment(
				origin: OriginFor<T>,
				receiver: T::AccountId,
				flexi_requested: u64,
				flexi_delivered: u64,
				price: u64,
				payment_type: u8
			) -> DispatchResult {
				// Ensure the caller is authorized to perform the action
				let sender = ensure_signed(origin.clone())?;

				// Get the parameters for the calculation
				let alpha = Alpha::<T>::get();
				let beta = Beta::<T>::get();
				let tolerance = Tolerance::<T>::get();
				
				// Fixed point calculations: 1.0 = 1_000_000
				let fixed_point_factor: u64 = 1_000_000; 
				
				// Calculate base payment (min of requested and delivered * price)
				let base = core::cmp::min(flexi_requested, flexi_delivered).saturating_mul(price);
				
				// Calculate tolerance threshold for requested flexibility
				let threshold = tolerance.saturating_mul(flexi_requested).checked_div(fixed_point_factor)
					.unwrap_or(0);
				
				// Under-delivery penalty
				let under_delivery_diff = flexi_requested.saturating_sub(flexi_delivered).saturating_sub(threshold);
				let under_delivery_penalty = if under_delivery_diff > 0 {
					alpha.saturating_mul(under_delivery_diff).saturating_mul(price)
						.checked_div(fixed_point_factor).unwrap_or(0)
				} else {
					0
				};
				
				// Over-delivery adjustment
				let over_delivery_diff = flexi_delivered.saturating_sub(flexi_requested).saturating_sub(threshold);
				let over_delivery_bonus = if over_delivery_diff > 0 {
					beta.saturating_mul(over_delivery_diff).saturating_mul(price)
						.checked_div(fixed_point_factor).unwrap_or(0)
				} else {
					0
				};
				
				// Calculate final amount
				let final_amount = base.saturating_sub(under_delivery_penalty)
					.saturating_add(over_delivery_bonus);
				
				// Convert to BalanceOf<T>
				let amount = BalanceOf::<T>::from(final_amount as u32);
				
				// Call the existing add_payment function to process the payment
				Self::add_payment(origin, receiver.clone(), amount, payment_type)?;
				
				// Emit the FlexibilitySettled event
				Self::deposit_event(Event::FlexibilitySettled {
					requester: sender,
					provider: receiver,
					requested: flexi_requested,
					delivered: flexi_delivered,
					price,
					calculated_amount: amount,
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
			pub fn query_community_info(community_id: T::AccountId) -> Option<CommunityInfo<T>> {
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

			/// Query the alpha parameter value.
			///
			/// - **Returns**:
			///   - The current alpha parameter value.
			pub fn query_alpha() -> u64 {
				Self::alpha()
			}

			/// Query the beta parameter value.
			///
			/// - **Returns**:
			///   - The current beta parameter value.
			pub fn query_beta() -> u64 {
				Self::beta()
			}

			/// Query the tolerance parameter value.
			///
			/// - **Returns**:
			///   - The current tolerance parameter value.
			pub fn query_tolerance() -> u64 {
				Self::tolerance()
			}			
		}
	}
