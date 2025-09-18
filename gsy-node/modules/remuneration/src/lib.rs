// Copyright (C) SUPSI-DACD-ISAAC (www.supsi.ch/isaac)
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
		use sp_std::marker::PhantomData;

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
		pub struct Pallet<T>(PhantomData<T>);

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

		/// ## Under Tolerance Parameter
		/// Used in settlement calculations for acceptable under-delivery deviation thresholds.
		/// This is a fixed-point representation where 1.0 = 1_000_000
		#[pallet::storage]
		#[pallet::getter(fn under_tolerance)]
		pub(super) type UnderTolerance<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// ## Over Tolerance Parameter
		/// Used in settlement calculations for acceptable over-delivery deviation thresholds.
		/// This is a fixed-point representation where 1.0 = 1_000_000
		#[pallet::storage]
		#[pallet::getter(fn over_tolerance)]
		pub(super) type OverTolerance<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// --- Adaptive parameters storage ---
		/// Reference benchmark for under-delivery deviation (fixed-point, 1.0 = 1_000_000)
		#[pallet::storage]
		#[pallet::getter(fn u_ref)]
		pub(super) type URef<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// Reference benchmark for over-delivery deviation (fixed-point, 1.0 = 1_000_000)
		#[pallet::storage]
		#[pallet::getter(fn o_ref)]
		pub(super) type ORef<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// Gain factor for alpha adaptation (fixed-point, 1.0 = 1_000_000).
		#[pallet::storage]
		#[pallet::getter(fn k_alpha)]
		pub(super) type KAlpha<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// Gain factor for beta adaptation (fixed-point, 1.0 = 1_000_000).
		#[pallet::storage]
		#[pallet::getter(fn k_beta)]
		pub(super) type KBeta<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// Gain factor for under-delivery tolerance adaptation (fixed-point, 1.0 = 1_000_000).
		#[pallet::storage]
		#[pallet::getter(fn k_under_tol)]
		pub(super) type KUnderTol<T: Config> = StorageValue<_, u64, ValueQuery>;

		/// Window size N for computing averages; must be > 0 and set by custodian.
		#[pallet::storage]
		#[pallet::getter(fn adaptation_window_size)]
		pub(super) type AdaptationWindowSize<T: Config> = StorageValue<_, u32, ValueQuery>;

		/// Piecewise settlement parameters
		#[pallet::storage]
		#[pallet::getter(fn alpha_piecewise)]
		pub(super) type AlphaPiecewise<T: Config> = StorageValue<_, u64, ValueQuery>;
		#[pallet::storage]
		#[pallet::getter(fn eps_piecewise_1)]
		pub(super) type EpsPiecewise1<T: Config> = StorageValue<_, u64, ValueQuery>;
		#[pallet::storage]
		#[pallet::getter(fn eps_piecewise_2)]
		pub(super) type EpsPiecewise2<T: Config> = StorageValue<_, u64, ValueQuery>;

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

			/// Emitted when the Under-Delivery Tolerance parameter is updated.
			/// - `old_value`: The previous value.
			/// - `new_value`: The new value.
			UnderToleranceUpdated { old_value: u64, new_value: u64 },

			/// Emitted when the Over-Delivery Tolerance parameter is updated.
			/// - `old_value`: The previous value.
			/// - `new_value`: The new value.
			OverToleranceUpdated { old_value: u64, new_value: u64 },

			/// Emitted when a flexibility settlement is processed.
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

			/// Emitted when adaptation policy parameters are updated by the custodian.
			AdaptationParamsUpdated {
				u_ref: u64,
				o_ref: u64,
				k_alpha: u64,
				k_beta: u64,
				k_under_tol: u64,
				window_size: u32,
			},

			/// Emitted when alpha and beta are adapted based on recent measurements.
			AlphaBetaAdapted {
				old_alpha: u64,
				new_alpha: u64,
				old_beta: u64,
				new_beta: u64,
				u_avg: u64,
				o_avg: u64,
			},

			/// Emitted when alpha_piecewise is updated.
			AlphaPiecewiseUpdated { old_value: u64, new_value: u64 },
			/// Emitted when eps_piecewise_1 is updated.
			EpsPiecewise1Updated { old_value: u64, new_value: u64 },
			/// Emitted when eps_piecewise_2 is updated.
			EpsPiecewise2Updated { old_value: u64, new_value: u64 },
		}

		#[pallet::error]
		pub enum Error<T> {
			NotCustodian,
			NotAllowedToManageProsumers,
			SameSenderReceiver,
			InsufficientBalance,
			SenderNotProsumer,
			ReceiverNotProsumer,
			DifferentCommunities,
			NotACommunity,
			NotCommunityOwner,
			PaymentTypeNotAllowed,
			InvalidWindowSize,
			EmptyMeasurements,
			MeasurementsExceedWindow,
			MismatchedMeasurements,
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
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_under_tolerance())]
			#[pallet::call_index(15)]
			pub fn update_under_tolerance(origin: OriginFor<T>, new_value: u64) -> DispatchResult {
				let sender = ensure_signed(origin)?;
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);
				let old = UnderTolerance::<T>::get();
				UnderTolerance::<T>::put(new_value);
				Self::deposit_event(Event::UnderToleranceUpdated { old_value: old, new_value });
				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::update_over_tolerance())]
			#[pallet::call_index(16)]
			pub fn update_over_tolerance(origin: OriginFor<T>, new_value: u64) -> DispatchResult {
				let sender = ensure_signed(origin)?;
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);
				let old = OverTolerance::<T>::get();
				OverTolerance::<T>::put(new_value);
				Self::deposit_event(Event::OverToleranceUpdated { old_value: old, new_value });
				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::settle_flexibility_payment())]
			#[pallet::call_index(17)]
			pub fn settle_flexibility_payment(
				origin: OriginFor<T>,
				receiver: T::AccountId,
				flexi_requested: u64,
				flexi_delivered: u64,
				price: u64,
				payment_type: u8
			) -> DispatchResult {
				let sender = ensure_signed(origin.clone())?;

				// Parameters
				let alpha = Alpha::<T>::get();
				let beta = Beta::<T>::get();
				let under_tol = UnderTolerance::<T>::get();
				let over_tol = OverTolerance::<T>::get();
				let f: u64 = 1_000_000;

				let base = core::cmp::min(flexi_requested, flexi_delivered).saturating_mul(price);
				let threshold_under = under_tol.saturating_mul(flexi_requested).checked_div(f).unwrap_or(0);
				let threshold_over = over_tol.saturating_mul(flexi_requested).checked_div(f).unwrap_or(0);
				let under_diff = flexi_requested.saturating_sub(flexi_delivered).saturating_sub(threshold_under);
				let under_penalty = if under_diff > 0 { alpha.saturating_mul(under_diff).saturating_mul(price).checked_div(f).unwrap_or(0) } else { 0 };
				let over_diff = flexi_delivered.saturating_sub(flexi_requested).saturating_sub(threshold_over);
				let over_bonus = if over_diff > 0 { beta.saturating_mul(over_diff).saturating_mul(price).checked_div(f).unwrap_or(0) } else { 0 };
				let final_amount = base.saturating_sub(under_penalty).saturating_add(over_bonus);
				let amount = BalanceOf::<T>::from(final_amount as u32);
				Self::add_payment(origin, receiver.clone(), amount, payment_type)?;
				Self::deposit_event(Event::FlexibilitySettled { requester: sender, provider: receiver, requested: flexi_requested, delivered: flexi_delivered, price, calculated_amount: amount });
				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::settle_flexibility_payment_with_pw_quad_penalty())]
			#[pallet::call_index(23)]
			pub fn settle_flexibility_payment_with_pw_quad_penalty(
				origin: OriginFor<T>,
				receiver: T::AccountId,
				flexi_requested: u64,
				flexi_delivered: u64,
				price: u64,
				payment_type: u8
			) -> DispatchResult {
				let sender = ensure_signed(origin.clone())?;

				// Base payment is for energy actually delivered up to the requested amount
				let base: u128 = (core::cmp::min(flexi_requested, flexi_delivered) as u128)
					.saturating_mul(price as u128);
				// Penalty computed via piecewise quadratic policy (energy units), then converted to value with price
				let penalty_energy: u128 = Self::calc_piecewise_quadratic_penalty(flexi_requested, flexi_delivered) as u128;
				let penalty_value: u128 = penalty_energy.saturating_mul(price as u128);
				let final_amount_u128 = base.saturating_sub(penalty_value);
				let final_amount_u64 = final_amount_u128.min(u128::from(u64::MAX)) as u64;
				let amount = BalanceOf::<T>::from(final_amount_u64 as u32);

				Self::add_payment(origin, receiver.clone(), amount, payment_type)?;
				Self::deposit_event(Event::FlexibilitySettled { requester: sender, provider: receiver, requested: flexi_requested, delivered: flexi_delivered, price, calculated_amount: amount });
				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::set_adaptation_params())]
			#[pallet::call_index(18)]
			pub fn set_adaptation_params(
				origin: OriginFor<T>, u_ref: u64, o_ref: u64, k_alpha: u64, k_beta: u64, k_under_tol: u64, window_size: u32,
			) -> DispatchResult {
				let sender = ensure_signed(origin)?;
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);
				ensure!(window_size > 0, Error::<T>::InvalidWindowSize);
				URef::<T>::put(u_ref); ORef::<T>::put(o_ref); KAlpha::<T>::put(k_alpha); KBeta::<T>::put(k_beta); KUnderTol::<T>::put(k_under_tol); AdaptationWindowSize::<T>::put(window_size);
				Self::deposit_event(Event::AdaptationParamsUpdated { u_ref, o_ref, k_alpha, k_beta, k_under_tol, window_size });
				Ok(())
			}

			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::dynamically_adapt_parameters())]
			#[pallet::call_index(19)]
			pub fn dynamically_adapt_parameters(origin: OriginFor<T>, u_measurements: Vec<u64>, o_measurements: Vec<u64>) -> DispatchResult {
				let sender = ensure_signed(origin)?; ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);
				let n_cfg = AdaptationWindowSize::<T>::get(); ensure!(n_cfg > 0, Error::<T>::InvalidWindowSize);
				let n_u = u_measurements.len() as u32; let n_o = o_measurements.len() as u32; ensure!(n_u > 0 && n_o > 0, Error::<T>::EmptyMeasurements); ensure!(n_u == n_o, Error::<T>::MismatchedMeasurements); ensure!(n_u == n_cfg, Error::<T>::MeasurementsExceedWindow);
				let sum_u: u128 = u_measurements.iter().fold(0u128, |a,v| a.saturating_add(*v as u128));
				let sum_o: u128 = o_measurements.iter().fold(0u128, |a,v| a.saturating_add(*v as u128));
				let n: u128 = n_u as u128; let u_avg = (sum_u / n) as u64; let o_avg = (sum_o / n) as u64;
				let alpha = Alpha::<T>::get(); let beta = Beta::<T>::get(); let under_old = UnderTolerance::<T>::get();
				let u_ref = URef::<T>::get(); let o_ref = ORef::<T>::get(); let k_alpha = KAlpha::<T>::get(); let k_beta = KBeta::<T>::get(); let k_under = KUnderTol::<T>::get();
				let f: i128 = 1_000_000;
				// Alpha adaptation
				let delta_u = (u_avg as i128) - (u_ref as i128); let factor_a = f + ( (k_alpha as i128).saturating_mul(delta_u) ) / f; let mut new_alpha_i = (alpha as i128).saturating_mul(factor_a).checked_div(f).unwrap_or(0); if new_alpha_i < 0 { new_alpha_i = 0; } let new_alpha: u64 = new_alpha_i.clamp(0, u64::MAX as i128) as u64;
				// Beta adaptation
				let delta_o = (o_avg as i128) - (o_ref as i128); let factor_b = f + ( (k_beta as i128).saturating_mul(delta_o) ) / f; let mut new_beta_i = (beta as i128).saturating_mul(factor_b).checked_div(f).unwrap_or(0); if new_beta_i < 0 { new_beta_i = 0; } let new_beta: u64 = new_beta_i.clamp(0, u64::MAX as i128) as u64;
				// Under tolerance adaptation: under_next = under_old * (1 - k_under * (u_avg - u_ref))
				let factor_ut = f - ( (k_under as i128).saturating_mul(delta_u) ) / f; // 1 - k*(delta_u)
				let mut new_ut_i = (under_old as i128).saturating_mul(factor_ut).checked_div(f).unwrap_or(0);
				if new_ut_i < 0 { new_ut_i = 0; }
				let new_under: u64 = new_ut_i.clamp(0, u64::MAX as i128) as u64;
				// Persist
				Alpha::<T>::put(new_alpha); Beta::<T>::put(new_beta); UnderTolerance::<T>::put(new_under);
				// Events
				Self::deposit_event(Event::AlphaUpdated { old_value: alpha, new_value: new_alpha });
				Self::deposit_event(Event::BetaUpdated { old_value: beta, new_value: new_beta });
				if new_under != under_old { Self::deposit_event(Event::UnderToleranceUpdated { old_value: under_old, new_value: new_under }); }
				Self::deposit_event(Event::AlphaBetaAdapted { old_alpha: alpha, new_alpha, old_beta: beta, new_beta, u_avg, o_avg });
				Ok(())
			}

			/// ## Update Piecewise Parameters
			///
			/// Updates all piecewise parameters used for flexible settlement calculations.
			/// - `new_alpha_pw`: New alpha_piecewise value (dimensionless, integer scaling factor)
			/// - `new_eps1`: New eps_piecewise_1 value (fixed-point 1e6)
			/// - `new_eps2`: New eps_piecewise_2 value (fixed-point 1e6)
			///
			/// Access: Custodian only
			#[transactional]
			#[pallet::weight(<T as Config>::RemunerationWeightInfo::set_piecewise_parameters())]
			#[pallet::call_index(20)]
			pub fn set_piecewise_parameters(origin: OriginFor<T>, new_alpha_pw: u64, new_eps1: u64, new_eps2: u64) -> DispatchResult {
				let sender = ensure_signed(origin)?;
				ensure!(Some(sender) == Custodian::<T>::get(), Error::<T>::NotCustodian);
				let old_alpha = AlphaPiecewise::<T>::get();
				let old_eps1 = EpsPiecewise1::<T>::get();
				let old_eps2 = EpsPiecewise2::<T>::get();
				AlphaPiecewise::<T>::put(new_alpha_pw);
				EpsPiecewise1::<T>::put(new_eps1);
				EpsPiecewise2::<T>::put(new_eps2);
				// Emit the three existing events for compatibility with downstream listeners
				Self::deposit_event(Event::AlphaPiecewiseUpdated { old_value: old_alpha, new_value: new_alpha_pw });
				Self::deposit_event(Event::EpsPiecewise1Updated { old_value: old_eps1, new_value: new_eps1 });
				Self::deposit_event(Event::EpsPiecewise2Updated { old_value: old_eps2, new_value: new_eps2 });
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
			/// Calculate piecewise quadratic under-delivery penalty based on global parameters.
			/// Inputs:
			/// - flexi_requested (E_r)
			/// - flexi_delivered (E_m)
			/// Global params (fixed-point 1e6):
			/// - alpha_piecewise, eps_piecewise_1, eps_piecewise_2
			/// Piecewise rule:
			/// e1 = E_r * (1 - eps1)
			/// e2 = E_r * (1 - eps2)
			/// if E_m >= e1: 0
			/// else if e2 <= E_m < e1: alpha*(e1 - E_m)
			/// else (E_m < e2): alpha*(e1 - E_m) + alpha*(e2 - E_m)^2
			pub fn calc_piecewise_quadratic_penalty(flexi_requested: u64, flexi_delivered: u64) -> u64 {
				let f: u128 = 1_000_000u128;
				let er: u128 = flexi_requested as u128;
				let em: u128 = flexi_delivered as u128;
				let alpha: u128 = AlphaPiecewise::<T>::get() as u128;
				let eps1: u128 = EpsPiecewise1::<T>::get() as u128;
				let eps2: u128 = EpsPiecewise2::<T>::get() as u128;
				// e1 = Er * (1 - eps1); e2 = Er * (1 - eps2) with fixed-point eps
				let one_minus_eps1 = f.saturating_sub(eps1);
				let one_minus_eps2 = f.saturating_sub(eps2);
				let e1: u128 = one_minus_eps1.saturating_mul(er).checked_div(f).unwrap_or(0);
				let e2: u128 = one_minus_eps2.saturating_mul(er).checked_div(f).unwrap_or(0);
				if em >= e1 {
					return 0;
				}
				let diff1 = e1.saturating_sub(em);
				let mut penalty: u128 = alpha.saturating_mul(diff1);
				if em < e2 {
					let diff2 = e2.saturating_sub(em);
					let quad = alpha.saturating_mul(diff2.saturating_mul(diff2));
					penalty = penalty.saturating_add(quad);
				}
				penalty.min(u128::from(u64::MAX)) as u64
			}

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
			pub fn query_under_tolerance() -> u64 { Self::under_tolerance() }
			pub fn query_over_tolerance() -> u64 { Self::over_tolerance() }
			/// Query adaptation parameters.
			pub fn query_adaptation_params() -> (u64, u64, u64, u64, u32) {
				(
					Self::u_ref(),
					Self::o_ref(),
					Self::k_alpha(),
					Self::k_beta(),
					Self::adaptation_window_size(),
				)
			}
		}
	}
