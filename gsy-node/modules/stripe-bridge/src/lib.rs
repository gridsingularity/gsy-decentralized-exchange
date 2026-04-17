// Copyright (C) SUPSI-DACD-ISAAC (www.supsi.ch/isaac)
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! # Stripe Bridge Pallet
//!
//! An offchain worker pallet that bridges on-chain remuneration payments
//! to the Stripe payment platform. It mirrors the operations from the
//! stripe-testbed reference (create payment, get balance, refund, etc.)
//! adapted for the Substrate offchain worker context.
//!
//! ## Architecture
//!
//! - **On-chain**: queues payment/refund requests and stores results
//! - **Offchain worker**: processes queued requests via Stripe HTTP API
//! - **Integration**: reads from the remuneration pallet for balance validation

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame_support::sp_runtime::transaction_validity::{TransactionValidity, ValidTransaction};
use sp_core::crypto::KeyTypeId;

pub use crate::weights::WeightInfo;
pub use pallet::*;

pub mod stripe_client;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

/// Key type for the stripe-bridge offchain worker crypto identity.
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"strp");

/// Offchain local storage key for the Stripe API key.
pub const STRIPE_API_KEY_STORAGE: &[u8] = b"stripe-bridge::api-key";

/// Offchain local storage key for a pending balance-check flag.
pub const BALANCE_CHECK_FLAG: &[u8] = b"stripe-bridge::balance-check";

/// Offchain local storage key prefix for canonical outbound transfer in-flight guards.
pub const OUTBOUND_TRANSFER_IN_FLIGHT_PREFIX: &[u8] = b"stripe-bridge::outbound-transfer-in-flight";

/// Cooldown window for canonical outbound transfer in-flight guards.
pub const OUTBOUND_TRANSFER_IN_FLIGHT_TTL_MS: u64 = 60_000;

pub mod crypto {
	use super::KEY_TYPE;
	use scale_info::prelude::string::String;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::stripe_client;
	use alloc::format;
	use codec::{Decode, Encode};
	use frame_support::{pallet_prelude::*, transactional};
	use frame_system::{
		offchain::{AppCrypto, SendUnsignedTransaction, SignedPayload, Signer, SigningTypes},
		pallet_prelude::*,
	};
	use sp_runtime::traits::SaturatedConversion;
	use sp_std::vec::Vec;

	// -----------------------------------------------------------------------
	// Types stored on-chain
	// -----------------------------------------------------------------------

	/// A payment request queued for Stripe processing.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct StripePaymentRequest<AccountId> {
		pub receiver: AccountId,
		pub amount: u64,
		pub currency: BoundedVec<u8, ConstU32<8>>,
	}

	/// The recorded result of a completed Stripe PaymentIntent.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct StripePaymentRecord {
		pub stripe_payment_id: BoundedVec<u8, ConstU32<128>>,
		pub status: BoundedVec<u8, ConstU32<32>>,
		pub gross_amount: u64,
		pub stripe_fee: u64,
		pub net_amount: u64,
	}

	/// A refund request queued for Stripe processing.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct StripeRefundRequest {
		pub payment_index: u64,
		pub stripe_payment_id: BoundedVec<u8, ConstU32<128>>,
	}

	/// The recorded result of a completed Stripe refund.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct StripeRefundRecord {
		pub refund_id: BoundedVec<u8, ConstU32<128>>,
		pub status: BoundedVec<u8, ConstU32<32>>,
		pub amount: u64,
	}

	/// Balance snapshot from Stripe.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct StripeBalanceInfo {
		pub available_amount: i64,
		pub available_currency: BoundedVec<u8, ConstU32<8>>,
		pub pending_amount: i64,
		pub pending_currency: BoundedVec<u8, ConstU32<8>>,
	}

	/// Direction of a canonical bridge transfer.
	#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum BridgeTransferDirection {
		ToStripe,
		FromStripe,
	}

	/// Lifecycle state of a canonical bridge transfer.
	#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum BridgeTransferStatus {
		Requested,
		FundsReserved,
		SubmittedToStripe,
		AwaitingConfirmation,
		Succeeded,
		Failed,
		Reverted,
		CreditedOnChain,
		Finalized,
	}

	/// Canonical bridge transfer record kept alongside the legacy queue model.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct BridgeTransfer<AccountId> {
		pub owner: AccountId,
		pub amount: u64,
		pub currency: BoundedVec<u8, ConstU32<8>>,
		pub direction: BridgeTransferDirection,
		pub status: BridgeTransferStatus,
		pub retry_of: Option<u64>,
		pub stripe_object_id: Option<BoundedVec<u8, ConstU32<128>>>,
		pub external_reference: Option<BoundedVec<u8, ConstU32<128>>>,
		pub escrow_reference: Option<BoundedVec<u8, ConstU32<128>>>,
		pub last_error: Option<BoundedVec<u8, ConstU32<256>>>,
	}

	// -----------------------------------------------------------------------
	// Unsigned-transaction payloads
	// -----------------------------------------------------------------------

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
	pub struct PaymentResultPayload<Public> {
		pub payment_index: u64,
		pub stripe_payment_id: Vec<u8>,
		pub status: Vec<u8>,
		pub gross_amount: u64,
		pub stripe_fee: u64,
		pub net_amount: u64,
		pub public: Public,
	}

	impl<T: SigningTypes> SignedPayload<T> for PaymentResultPayload<T::Public> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
	pub struct RefundResultPayload<Public> {
		pub refund_index: u64,
		pub refund_id: Vec<u8>,
		pub status: Vec<u8>,
		pub amount: u64,
		pub public: Public,
	}

	impl<T: SigningTypes> SignedPayload<T> for RefundResultPayload<T::Public> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
	pub struct BalanceResultPayload<Public> {
		pub available_amount: i64,
		pub available_currency: Vec<u8>,
		pub pending_amount: i64,
		pub pending_currency: Vec<u8>,
		pub public: Public,
	}

	impl<T: SigningTypes> SignedPayload<T> for BalanceResultPayload<T::Public> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
	pub struct OutboundTransferResultPayload<Public> {
		pub bridge_id: u64,
		pub success: bool,
		pub stripe_object_id: Vec<u8>,
		pub stripe_status: Vec<u8>,
		pub error_message: Vec<u8>,
		pub public: Public,
	}

	impl<T: SigningTypes> SignedPayload<T> for OutboundTransferResultPayload<T::Public> {
		fn public(&self) -> T::Public {
			self.public.clone()
		}
	}

	// -----------------------------------------------------------------------
	// Pallet configuration
	// -----------------------------------------------------------------------

	#[pallet::config]
	pub trait Config:
		frame_system::offchain::CreateSignedTransaction<Call<Self>>
		+ frame_system::offchain::SendTransactionTypes<Call<Self>>
		+ frame_system::Config
		+ remuneration::Config
	{
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		type RuntimeEvent: From<Event<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>
			+ Into<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: From<Call<Self>> + Into<<Self as frame_system::Config>::RuntimeCall>;

		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// -----------------------------------------------------------------------
	// Storage
	// -----------------------------------------------------------------------

	/// Payments queued for Stripe processing.
	#[pallet::storage]
	#[pallet::getter(fn pending_payments)]
	pub type PendingPayments<T: Config> =
		StorageMap<_, Twox64Concat, u64, StripePaymentRequest<T::AccountId>, OptionQuery>;

	/// Completed payment results from Stripe.
	#[pallet::storage]
	#[pallet::getter(fn processed_payments)]
	pub type ProcessedPayments<T: Config> =
		StorageMap<_, Twox64Concat, u64, StripePaymentRecord, OptionQuery>;

	/// Auto-incrementing index for payment requests.
	#[pallet::storage]
	#[pallet::getter(fn next_payment_index)]
	pub type NextPaymentIndex<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Refunds queued for Stripe processing.
	#[pallet::storage]
	#[pallet::getter(fn pending_refunds)]
	pub type PendingRefunds<T: Config> =
		StorageMap<_, Twox64Concat, u64, StripeRefundRequest, OptionQuery>;

	/// Completed refund results from Stripe.
	#[pallet::storage]
	#[pallet::getter(fn processed_refunds)]
	pub type ProcessedRefunds<T: Config> =
		StorageMap<_, Twox64Concat, u64, StripeRefundRecord, OptionQuery>;

	/// Auto-incrementing index for refund requests.
	#[pallet::storage]
	#[pallet::getter(fn next_refund_index)]
	pub type NextRefundIndex<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Whether the Stripe bridge is enabled.
	#[pallet::storage]
	#[pallet::getter(fn stripe_enabled)]
	pub type StripeEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// Auto-incrementing identifier for canonical bridge transfers.
	#[pallet::storage]
	#[pallet::getter(fn next_bridge_transfer_id)]
	pub type NextBridgeTransferId<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Canonical bridge transfer records. Legacy payment/refund queues remain in place for
	/// compatibility until the settlement flow is migrated incrementally.
	#[pallet::storage]
	#[pallet::getter(fn bridge_transfers)]
	pub type BridgeTransfers<T: Config> =
		StorageMap<_, Twox64Concat, u64, BridgeTransfer<T::AccountId>, OptionQuery>;

	/// Last balance snapshot from Stripe.
	#[pallet::storage]
	#[pallet::getter(fn last_balance)]
	pub type LastBalance<T: Config> = StorageValue<_, StripeBalanceInfo, OptionQuery>;

	/// Flag indicating a balance check has been requested.
	#[pallet::storage]
	#[pallet::getter(fn balance_check_requested)]
	pub type BalanceCheckRequested<T: Config> = StorageValue<_, bool, ValueQuery>;

	// -----------------------------------------------------------------------
	// Events
	// -----------------------------------------------------------------------

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Stripe bridge enabled or disabled.
		StripeToggled { enabled: bool },

		/// A payment has been queued for Stripe processing.
		StripePaymentQueued { index: u64, receiver: T::AccountId, amount: u64, currency: Vec<u8> },

		/// A Stripe payment was processed successfully.
		StripePaymentProcessed {
			index: u64,
			stripe_payment_id: Vec<u8>,
			status: Vec<u8>,
			gross_amount: u64,
			stripe_fee: u64,
			net_amount: u64,
		},

		/// A Stripe payment processing failed.
		StripePaymentFailed { index: u64, reason: Vec<u8> },

		/// A refund has been queued for Stripe processing.
		StripeRefundQueued { index: u64, payment_index: u64 },

		/// A Stripe refund was processed successfully.
		StripeRefundProcessed { index: u64, refund_id: Vec<u8>, status: Vec<u8>, amount: u64 },

		/// A balance check has been requested.
		BalanceCheckRequested,

		/// A balance snapshot was recorded.
		StripeBalanceUpdated {
			available_amount: i64,
			available_currency: Vec<u8>,
			pending_amount: i64,
			pending_currency: Vec<u8>,
		},

		/// A canonical bridge transfer was created.
		BridgeTransferCreated {
			bridge_id: u64,
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			direction: BridgeTransferDirection,
			status: BridgeTransferStatus,
		},

		/// A canonical bridge transfer status was updated.
		BridgeTransferStatusUpdated {
			bridge_id: u64,
			old_status: BridgeTransferStatus,
			new_status: BridgeTransferStatus,
		},

		/// An outbound canonical bridge transfer was requested and reserved in remuneration.
		OutboundTransferToStripeRequested {
			bridge_id: u64,
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
		},

		/// An outbound canonical bridge transfer was finalized after Stripe success.
		OutboundTransferToStripeSucceeded { bridge_id: u64, stripe_object_id: Vec<u8> },

		/// An outbound canonical bridge transfer was reverted after Stripe failure.
		OutboundTransferToStripeFailed { bridge_id: u64, reason: Vec<u8> },

		/// A trusted inbound Stripe transfer confirmation was accepted.
		InboundTransferFromStripeConfirmed {
			bridge_id: u64,
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			external_reference: Vec<u8>,
		},

		/// An inbound Stripe transfer was credited on-chain.
		InboundTransferFromStripeCredited {
			bridge_id: u64,
			external_reference: Vec<u8>,
			stripe_object_id: Vec<u8>,
		},

		/// A failed outbound transfer was retried through a fresh canonical transfer.
		OutboundTransferRetried { original_bridge_id: u64, retry_bridge_id: u64 },

		/// A stuck outbound transfer was force-reverted by the custodian.
		OutboundTransferForceReverted { bridge_id: u64, reason: Vec<u8> },
	}

	// -----------------------------------------------------------------------
	// Errors
	// -----------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Stripe bridge is not enabled.
		StripeNotEnabled,
		/// Caller is not the remuneration custodian.
		NotCustodian,
		/// The payment index does not exist in processed payments.
		PaymentNotFound,
		/// The referenced processed payment has no Stripe ID.
		NoStripePaymentId,
		/// Offchain worker error when sending unsigned transaction.
		OffchainUnsignedTxError,
		/// Offchain worker: no local account for signing.
		NoLocalAcctForSigning,
		/// Currency string too long for bounded storage.
		CurrencyTooLong,
		/// Stripe payment ID too long for bounded storage.
		StripeIdTooLong,
		/// Status string too long.
		StatusTooLong,
		/// Refund ID too long.
		RefundIdTooLong,
		/// Canonical bridge transfer not found.
		BridgeTransferNotFound,
		/// Invalid lifecycle transition for a canonical bridge transfer.
		InvalidBridgeTransferStatusTransition,
		/// Canonical bridge transfer field too long for bounded storage.
		BridgeFieldTooLong,
		/// Canonical bridge transfer direction is incompatible with the requested action.
		InvalidBridgeTransferDirection,
		/// The inbound Stripe reference was already credited in remuneration.
		DuplicateInboundExternalReference,
		/// The outbound transfer is not in a retryable state.
		OutboundTransferNotRetryable,
		/// The outbound transfer is not in a force-revertable state.
		OutboundTransferNotForceRevertable,
	}

	// -----------------------------------------------------------------------
	// Hooks (offchain worker entry point)
	// -----------------------------------------------------------------------

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_block_number: BlockNumberFor<T>) {
			if !StripeEnabled::<T>::get() {
				return;
			}
			log::info!("[stripe-bridge] offchain worker starting");

			let api_key = match Self::read_api_key() {
				Some(k) => k,
				None => {
					log::warn!("[stripe-bridge] no API key in offchain storage");
					return;
				},
			};
			let api_key_str = match sp_std::str::from_utf8(&api_key) {
				Ok(s) => s,
				Err(_) => {
					log::error!("[stripe-bridge] API key is not valid UTF-8");
					return;
				},
			};

			Self::process_outbound_bridge_transfers(api_key_str);
			Self::process_pending_payments(api_key_str);
			Self::process_pending_refunds(api_key_str);
			Self::process_balance_check(api_key_str);
		}
	}

	// -----------------------------------------------------------------------
	// Extrinsics
	// -----------------------------------------------------------------------

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Enable or disable the Stripe bridge.
		/// Only the remuneration custodian may call this.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::set_stripe_enabled())]
		#[pallet::call_index(0)]
		pub fn set_stripe_enabled(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			StripeEnabled::<T>::put(enabled);
			Self::deposit_event(Event::StripeToggled { enabled });
			Ok(())
		}

		/// Queue a payment for Stripe processing.
		/// Only the remuneration custodian may call this.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::queue_stripe_payment())]
		#[pallet::call_index(1)]
		pub fn queue_stripe_payment(
			origin: OriginFor<T>,
			receiver: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);

			let bounded_currency: BoundedVec<u8, ConstU32<8>> =
				currency.clone().try_into().map_err(|_| Error::<T>::CurrencyTooLong)?;

			let index = NextPaymentIndex::<T>::get();
			let request = StripePaymentRequest {
				receiver: receiver.clone(),
				amount,
				currency: bounded_currency,
			};
			PendingPayments::<T>::insert(index, request);
			NextPaymentIndex::<T>::put(index + 1);

			Self::deposit_event(Event::StripePaymentQueued { index, receiver, amount, currency });
			Ok(())
		}

		/// Queue a refund for a previously processed Stripe payment.
		/// Only the remuneration custodian may call this.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::queue_stripe_refund())]
		#[pallet::call_index(2)]
		pub fn queue_stripe_refund(origin: OriginFor<T>, payment_index: u64) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);

			let record =
				ProcessedPayments::<T>::get(payment_index).ok_or(Error::<T>::PaymentNotFound)?;
			ensure!(!record.stripe_payment_id.is_empty(), Error::<T>::NoStripePaymentId);

			let refund_index = NextRefundIndex::<T>::get();
			let request =
				StripeRefundRequest { payment_index, stripe_payment_id: record.stripe_payment_id };
			PendingRefunds::<T>::insert(refund_index, request);
			NextRefundIndex::<T>::put(refund_index + 1);

			Self::deposit_event(Event::StripeRefundQueued { index: refund_index, payment_index });
			Ok(())
		}

		/// Request that the offchain worker retrieves the current Stripe balance.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::request_balance_check())]
		#[pallet::call_index(3)]
		pub fn request_balance_check(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);
			BalanceCheckRequested::<T>::put(true);
			Self::deposit_event(Event::BalanceCheckRequested);
			Ok(())
		}

		/// Request an outbound remuneration-to-Stripe bridge transfer using the canonical model.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::request_transfer_to_stripe())]
		#[pallet::call_index(4)]
		pub fn request_transfer_to_stripe(
			origin: OriginFor<T>,
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);

			let bridge_id = Self::create_outbound_bridge_transfer(
				owner.clone(),
				amount,
				currency.clone(),
				None,
			)?;

			Self::deposit_event(Event::OutboundTransferToStripeRequested {
				bridge_id,
				owner,
				amount,
				currency,
			});
			Ok(())
		}

		/// Record a trusted inbound Stripe confirmation and credit remuneration exactly once.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::confirm_transfer_from_stripe())]
		#[pallet::call_index(5)]
		pub fn confirm_transfer_from_stripe(
			origin: OriginFor<T>,
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			external_reference: Vec<u8>,
			stripe_object_id: Vec<u8>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);

			let bridge_id = Self::create_bridge_transfer(
				owner.clone(),
				amount,
				currency.clone(),
				BridgeTransferDirection::FromStripe,
			)?;
			Self::attach_bridge_transfer_external_reference(bridge_id, external_reference.clone())?;
			Self::attach_bridge_transfer_stripe_object_id(bridge_id, stripe_object_id.clone())?;

			match remuneration::Pallet::<T>::bridge_credit_inbound(
				external_reference.clone(),
				owner.clone(),
				amount.saturated_into(),
			) {
				Ok(()) => {},
				Err(error)
					if error == remuneration::Error::<T>::BridgeDuplicateExternalCredit.into() =>
				{
					return Err(Error::<T>::DuplicateInboundExternalReference.into());
				},
				Err(error) => return Err(error),
			}

			Self::deposit_event(Event::InboundTransferFromStripeConfirmed {
				bridge_id,
				owner,
				amount,
				currency,
				external_reference: external_reference.clone(),
			});
			Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::CreditedOnChain)?;
			Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Finalized)?;
			Self::deposit_event(Event::InboundTransferFromStripeCredited {
				bridge_id,
				external_reference,
				stripe_object_id,
			});
			Ok(())
		}

		/// Retry a failed outbound transfer by creating a fresh canonical transfer.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::retry_transfer_to_stripe())]
		#[pallet::call_index(6)]
		pub fn retry_transfer_to_stripe(
			origin: OriginFor<T>,
			original_bridge_id: u64,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);
			ensure!(StripeEnabled::<T>::get(), Error::<T>::StripeNotEnabled);

			let original_transfer = Self::query_bridge_transfer(original_bridge_id)
				.ok_or(Error::<T>::BridgeTransferNotFound)?;
			ensure!(
				original_transfer.direction == BridgeTransferDirection::ToStripe,
				Error::<T>::InvalidBridgeTransferDirection
			);
			ensure!(
				Self::is_retryable_outbound_transfer(original_bridge_id, &original_transfer),
				Error::<T>::OutboundTransferNotRetryable
			);

			let retry_bridge_id = Self::create_outbound_bridge_transfer(
				original_transfer.owner,
				original_transfer.amount,
				original_transfer.currency.to_vec(),
				Some(original_bridge_id),
			)?;

			Self::deposit_event(Event::OutboundTransferRetried {
				original_bridge_id,
				retry_bridge_id,
			});
			Ok(())
		}

		/// Force-revert a stuck outbound transfer and release its remuneration escrow.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::force_revert_outbound_transfer())]
		#[pallet::call_index(7)]
		pub fn force_revert_outbound_transfer(
			origin: OriginFor<T>,
			bridge_id: u64,
			reason: Vec<u8>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(
				Some(sender) == remuneration::Pallet::<T>::query_custodian(),
				Error::<T>::NotCustodian
			);

			let transfer =
				Self::query_bridge_transfer(bridge_id).ok_or(Error::<T>::BridgeTransferNotFound)?;
			ensure!(
				transfer.direction == BridgeTransferDirection::ToStripe,
				Error::<T>::InvalidBridgeTransferDirection
			);
			ensure!(
				!Self::is_successful_terminal_bridge_status(&transfer),
				Error::<T>::OutboundTransferNotForceRevertable
			);
			ensure!(
				Self::is_force_revertable_outbound_transfer(&transfer),
				Error::<T>::OutboundTransferNotForceRevertable
			);

			if !reason.is_empty() {
				Self::attach_bridge_transfer_last_error(bridge_id, reason.clone())?;
			}

			match transfer.status {
				BridgeTransferStatus::FundsReserved => {
					remuneration::Pallet::<T>::bridge_release_funds(
						Self::bridge_transfer_escrow_reference(bridge_id),
					)?;
					Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Reverted)?;
				},
				BridgeTransferStatus::SubmittedToStripe => {
					remuneration::Pallet::<T>::bridge_release_funds(
						Self::bridge_transfer_escrow_reference(bridge_id),
					)?;
					Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Failed)?;
					Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Reverted)?;
				},
				_ => return Err(Error::<T>::OutboundTransferNotForceRevertable.into()),
			}

			Self::deposit_event(Event::OutboundTransferForceReverted { bridge_id, reason });
			Ok(())
		}

		// -------------------------------------------------------------------
		// Unsigned extrinsics (submitted by OCW)
		// -------------------------------------------------------------------

		/// Record a Stripe payment result (submitted by the offchain worker).
		#[pallet::weight(<T as Config>::WeightInfo::submit_payment_result())]
		#[pallet::call_index(10)]
		pub fn submit_payment_result(
			origin: OriginFor<T>,
			payload: PaymentResultPayload<T::Public>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin)?;

			let bounded_id: BoundedVec<u8, ConstU32<128>> = payload
				.stripe_payment_id
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::StripeIdTooLong)?;
			let bounded_status: BoundedVec<u8, ConstU32<32>> =
				payload.status.clone().try_into().map_err(|_| Error::<T>::StatusTooLong)?;

			PendingPayments::<T>::remove(payload.payment_index);

			let record = StripePaymentRecord {
				stripe_payment_id: bounded_id,
				status: bounded_status,
				gross_amount: payload.gross_amount,
				stripe_fee: payload.stripe_fee,
				net_amount: payload.net_amount,
			};
			ProcessedPayments::<T>::insert(payload.payment_index, record);

			Self::deposit_event(Event::StripePaymentProcessed {
				index: payload.payment_index,
				stripe_payment_id: payload.stripe_payment_id,
				status: payload.status,
				gross_amount: payload.gross_amount,
				stripe_fee: payload.stripe_fee,
				net_amount: payload.net_amount,
			});
			Ok(())
		}

		/// Record a Stripe refund result (submitted by the offchain worker).
		#[pallet::weight(<T as Config>::WeightInfo::submit_refund_result())]
		#[pallet::call_index(11)]
		pub fn submit_refund_result(
			origin: OriginFor<T>,
			payload: RefundResultPayload<T::Public>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin)?;

			let bounded_id: BoundedVec<u8, ConstU32<128>> =
				payload.refund_id.clone().try_into().map_err(|_| Error::<T>::RefundIdTooLong)?;
			let bounded_status: BoundedVec<u8, ConstU32<32>> =
				payload.status.clone().try_into().map_err(|_| Error::<T>::StatusTooLong)?;

			PendingRefunds::<T>::remove(payload.refund_index);

			let record = StripeRefundRecord {
				refund_id: bounded_id,
				status: bounded_status,
				amount: payload.amount,
			};
			ProcessedRefunds::<T>::insert(payload.refund_index, record);

			Self::deposit_event(Event::StripeRefundProcessed {
				index: payload.refund_index,
				refund_id: payload.refund_id,
				status: payload.status,
				amount: payload.amount,
			});
			Ok(())
		}

		/// Record a Stripe balance result (submitted by the offchain worker).
		#[pallet::weight(<T as Config>::WeightInfo::submit_balance_result())]
		#[pallet::call_index(12)]
		pub fn submit_balance_result(
			origin: OriginFor<T>,
			payload: BalanceResultPayload<T::Public>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin)?;

			BalanceCheckRequested::<T>::put(false);

			let info = StripeBalanceInfo {
				available_amount: payload.available_amount,
				available_currency: payload
					.available_currency
					.clone()
					.try_into()
					.unwrap_or_default(),
				pending_amount: payload.pending_amount,
				pending_currency: payload.pending_currency.clone().try_into().unwrap_or_default(),
			};
			LastBalance::<T>::put(info);

			Self::deposit_event(Event::StripeBalanceUpdated {
				available_amount: payload.available_amount,
				available_currency: payload.available_currency,
				pending_amount: payload.pending_amount,
				pending_currency: payload.pending_currency,
			});
			Ok(())
		}

		/// Record the result of an outbound canonical bridge transfer.
		#[transactional]
		#[pallet::weight(<T as Config>::WeightInfo::submit_outbound_transfer_result())]
		#[pallet::call_index(13)]
		pub fn submit_outbound_transfer_result(
			origin: OriginFor<T>,
			payload: OutboundTransferResultPayload<T::Public>,
			_signature: T::Signature,
		) -> DispatchResult {
			ensure_none(origin)?;
			Self::handle_outbound_transfer_result(
				payload.bridge_id,
				payload.success,
				payload.stripe_object_id,
				payload.stripe_status,
				payload.error_message,
			)
		}
	}

	// -----------------------------------------------------------------------
	// ValidateUnsigned
	// -----------------------------------------------------------------------

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("stripe-bridge")
					.priority(T::UnsignedPriority::get())
					.and_provides([&provide])
					.longevity(3)
					.propagate(true)
					.build()
			};

			match call {
				Call::submit_payment_result { ref payload, ref signature } => {
					if !<PaymentResultPayload<T::Public> as SignedPayload<T>>::verify::<
						T::AuthorityId,
					>(payload, signature.clone())
					{
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(Self::build_unsigned_provides_key(
						b"submit_payment_result",
						payload.payment_index,
					))
				},
				Call::submit_refund_result { ref payload, ref signature } => {
					if !<RefundResultPayload<T::Public> as SignedPayload<T>>::verify::<T::AuthorityId>(
						payload,
						signature.clone(),
					) {
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(Self::build_unsigned_provides_key(
						b"submit_refund_result",
						payload.refund_index,
					))
				},
				Call::submit_balance_result { ref payload, ref signature } => {
					if !<BalanceResultPayload<T::Public> as SignedPayload<T>>::verify::<
						T::AuthorityId,
					>(payload, signature.clone())
					{
						return InvalidTransaction::BadProof.into();
					}
					// Balance checks are still modeled as a singleton request/result flow.
					valid_tx(b"submit_balance_result".to_vec())
				},
				Call::submit_outbound_transfer_result { ref payload, ref signature } => {
					if !<OutboundTransferResultPayload<T::Public> as SignedPayload<T>>::verify::<
						T::AuthorityId,
					>(payload, signature.clone())
					{
						return InvalidTransaction::BadProof.into();
					}
					valid_tx(Self::build_unsigned_provides_key(
						b"submit_outbound_transfer_result",
						payload.bridge_id,
					))
				},
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	// -----------------------------------------------------------------------
	// Offchain worker helpers
	// -----------------------------------------------------------------------

	impl<T: Config> Pallet<T> {
		/// Read the Stripe API key from offchain persistent local storage.
		/// Stored as raw bytes (not SCALE-encoded) so node operators can set it
		/// via `sp_io::offchain::local_storage_set`.
		fn read_api_key() -> Option<Vec<u8>> {
			sp_io::offchain::local_storage_get(
				sp_core::offchain::StorageKind::PERSISTENT,
				STRIPE_API_KEY_STORAGE,
			)
			.filter(|k| !k.is_empty())
		}

		/// Process pending payments: iterate storage, call Stripe, submit results.
		fn process_outbound_bridge_transfers(api_key: &str) {
			for (bridge_id, transfer) in BridgeTransfers::<T>::iter() {
				if transfer.direction != BridgeTransferDirection::ToStripe
					|| transfer.status != BridgeTransferStatus::FundsReserved
				{
					continue;
				}
				if Self::has_fresh_outbound_transfer_in_flight_marker(bridge_id) {
					log::info!(
                            "[stripe-bridge] skipping outbound bridge transfer {} due to in-flight marker",
                            bridge_id
                        );
					continue;
				}

				let currency_str = match sp_std::str::from_utf8(transfer.currency.as_slice()) {
					Ok(s) => s,
					Err(_) => "chf",
				};
				let idempotency_key = Self::canonical_outbound_idempotency_key(bridge_id);
				let idempotency_key_str = match sp_std::str::from_utf8(&idempotency_key) {
					Ok(s) => s,
					Err(_) => continue,
				};
				Self::mark_outbound_transfer_in_flight(bridge_id);

				log::info!(
					"[stripe-bridge] processing outbound bridge transfer {} : {} {}",
					bridge_id,
					transfer.amount,
					currency_str
				);

				match stripe_client::create_payment_intent_with_idempotency(
					api_key,
					transfer.amount,
					currency_str,
					Some(idempotency_key_str),
				) {
					Ok((code, body)) if code >= 200 && code < 300 => {
						let stripe_object_id =
							stripe_client::extract_json_string(&body, "id").unwrap_or_default();
						let stripe_status =
							stripe_client::extract_json_string(&body, "status").unwrap_or_default();

						if stripe_status.as_slice() == b"succeeded" {
							Self::send_outbound_transfer_result(
								bridge_id,
								true,
								stripe_object_id,
								stripe_status,
								sp_std::vec![],
							);
						} else {
							Self::send_outbound_transfer_result(
								bridge_id,
								false,
								stripe_object_id,
								stripe_status,
								b"unexpected stripe status".to_vec(),
							);
						}
					},
					Ok((code, body)) => {
						log::warn!(
							"[stripe-bridge] Stripe error {} for outbound transfer {}",
							code,
							bridge_id
						);
						let error_message = stripe_client::extract_json_string(&body, "message")
							.unwrap_or_else(|| b"unknown error".to_vec());
						let stripe_status = stripe_client::extract_json_string(&body, "status")
							.unwrap_or_else(|| b"failed".to_vec());
						let stripe_object_id =
							stripe_client::extract_json_string(&body, "id").unwrap_or_default();

						Self::send_outbound_transfer_result(
							bridge_id,
							false,
							stripe_object_id,
							stripe_status,
							error_message,
						);
					},
					Err(e) => {
						log::error!(
							"[stripe-bridge] HTTP error for outbound transfer {}: {:?}",
							bridge_id,
							e
						);
						Self::send_outbound_transfer_result(
							bridge_id,
							false,
							sp_std::vec![],
							b"http_error".to_vec(),
							b"http_error".to_vec(),
						);
					},
				}

				break;
			}
		}

		/// Process pending payments: iterate storage, call Stripe, submit results.
		fn process_pending_payments(api_key: &str) {
			let mut processed = sp_std::vec::Vec::new();
			for (index, request) in PendingPayments::<T>::iter() {
				let currency_str = match sp_std::str::from_utf8(request.currency.as_slice()) {
					Ok(s) => s,
					Err(_) => "chf",
				};

				log::info!(
					"[stripe-bridge] processing payment {} : {} {}",
					index,
					request.amount,
					currency_str
				);

				match stripe_client::create_payment_intent(api_key, request.amount, currency_str) {
					Ok((code, body)) if code >= 200 && code < 300 => {
						let pi_id =
							stripe_client::extract_json_string(&body, "id").unwrap_or_default();
						let status =
							stripe_client::extract_json_string(&body, "status").unwrap_or_default();
						let gross =
							stripe_client::extract_json_i64(&body, "amount").unwrap_or(0) as u64;

						// Try to get fee details from expanded charge
						let (fee, net) = Self::fetch_transaction_details(api_key, &pi_id);

						processed.push((index, pi_id, status, gross, fee, net));
					},
					Ok((code, body)) => {
						log::warn!("[stripe-bridge] Stripe error {} for payment {}", code, index);
						let error_msg = stripe_client::extract_json_string(&body, "message")
							.unwrap_or_else(|| b"unknown error".to_vec());
						processed.push((index, sp_std::vec![], b"failed".to_vec(), 0, 0, 0));
						let _ = error_msg; // logged via the status
					},
					Err(e) => {
						log::error!("[stripe-bridge] HTTP error for payment {}: {:?}", index, e);
						processed.push((index, sp_std::vec![], b"http_error".to_vec(), 0, 0, 0));
					},
				}
				// Process at most one payment per block to keep OCW fast
				break;
			}

			for (index, pi_id, status, gross, fee, net) in processed {
				Self::send_payment_result(index, pi_id, status, gross, fee, net);
			}
		}

		/// Attempt to fetch balance_transaction details for fee breakdown.
		fn fetch_transaction_details(api_key: &str, pi_id: &[u8]) -> (u64, u64) {
			let pi_str = match sp_std::str::from_utf8(pi_id) {
				Ok(s) => s,
				Err(_) => return (0, 0),
			};
			match stripe_client::get_payment_details(api_key, pi_str) {
				Ok((code, body)) if code >= 200 && code < 300 => {
					let fee = stripe_client::extract_nested_json_i64(
						&body,
						"latest_charge",
						"balance_transaction",
					)
					.and_then(|_| {
						// Parse the nested structure more carefully
						let val: serde_json::Value = serde_json::from_slice(&body).ok()?;
						let bt = val.get("latest_charge")?.get("balance_transaction")?;
						let f = bt.get("fee")?.as_i64()?;
						Some(f as u64)
					})
					.unwrap_or(0);

					let net = {
						let val: serde_json::Value =
							serde_json::from_slice(&body).unwrap_or_default();
						val.get("latest_charge")
							.and_then(|c| c.get("balance_transaction"))
							.and_then(|bt| bt.get("net"))
							.and_then(|n| n.as_i64())
							.unwrap_or(0) as u64
					};

					(fee, net)
				},
				_ => (0, 0),
			}
		}

		/// Submit an unsigned transaction with a payment result.
		fn send_payment_result(
			index: u64,
			stripe_payment_id: Vec<u8>,
			status: Vec<u8>,
			gross_amount: u64,
			stripe_fee: u64,
			net_amount: u64,
		) {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			if let Some((_, res)) = signer.send_unsigned_transaction(
				|account| PaymentResultPayload {
					payment_index: index,
					stripe_payment_id: stripe_payment_id.clone(),
					status: status.clone(),
					gross_amount,
					stripe_fee,
					net_amount,
					public: account.public.clone(),
				},
				|payload, signature| Call::submit_payment_result { payload, signature },
			) {
				match res {
					Ok(_) => {
						log::info!("[stripe-bridge] submitted payment result for index {}", index)
					},
					Err(()) => {
						log::error!("[stripe-bridge] failed to submit payment result")
					},
				}
			} else {
				log::error!("[stripe-bridge] no local account for signing");
			}
		}

		fn send_outbound_transfer_result(
			bridge_id: u64,
			success: bool,
			stripe_object_id: Vec<u8>,
			stripe_status: Vec<u8>,
			error_message: Vec<u8>,
		) {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			if let Some((_, res)) = signer.send_unsigned_transaction(
				|account| OutboundTransferResultPayload {
					bridge_id,
					success,
					stripe_object_id: stripe_object_id.clone(),
					stripe_status: stripe_status.clone(),
					error_message: error_message.clone(),
					public: account.public.clone(),
				},
				|payload, signature| Call::submit_outbound_transfer_result { payload, signature },
			) {
				match res {
					Ok(_) => log::info!(
						"[stripe-bridge] submitted outbound transfer result for bridge id {}",
						bridge_id
					),
					Err(()) => {
						log::error!("[stripe-bridge] failed to submit outbound transfer result")
					},
				}
			} else {
				log::error!("[stripe-bridge] no local account for signing");
			}
		}

		/// Process pending refunds.
		fn process_pending_refunds(api_key: &str) {
			let mut processed = sp_std::vec::Vec::new();
			for (index, request) in PendingRefunds::<T>::iter() {
				let pi_id_str = match sp_std::str::from_utf8(request.stripe_payment_id.as_slice()) {
					Ok(s) => s,
					Err(_) => continue,
				};

				log::info!("[stripe-bridge] processing refund {} for PI {}", index, pi_id_str);

				match stripe_client::create_refund(api_key, pi_id_str) {
					Ok((code, body)) if code >= 200 && code < 300 => {
						let refund_id =
							stripe_client::extract_json_string(&body, "id").unwrap_or_default();
						let status =
							stripe_client::extract_json_string(&body, "status").unwrap_or_default();
						let amount =
							stripe_client::extract_json_i64(&body, "amount").unwrap_or(0) as u64;
						processed.push((index, refund_id, status, amount));
					},
					Ok((code, _body)) => {
						log::warn!(
							"[stripe-bridge] Stripe refund error {} for index {}",
							code,
							index
						);
						processed.push((index, sp_std::vec![], b"failed".to_vec(), 0));
					},
					Err(e) => {
						log::error!("[stripe-bridge] HTTP error for refund {}: {:?}", index, e);
						processed.push((index, sp_std::vec![], b"http_error".to_vec(), 0));
					},
				}
				break; // one per block
			}

			for (index, refund_id, status, amount) in processed {
				Self::send_refund_result(index, refund_id, status, amount);
			}
		}

		/// Submit an unsigned transaction with a refund result.
		fn send_refund_result(refund_index: u64, refund_id: Vec<u8>, status: Vec<u8>, amount: u64) {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			if let Some((_, res)) = signer.send_unsigned_transaction(
				|account| RefundResultPayload {
					refund_index,
					refund_id: refund_id.clone(),
					status: status.clone(),
					amount,
					public: account.public.clone(),
				},
				|payload, signature| Call::submit_refund_result { payload, signature },
			) {
				match res {
					Ok(_) => log::info!(
						"[stripe-bridge] submitted refund result for index {}",
						refund_index
					),
					Err(()) => {
						log::error!("[stripe-bridge] failed to submit refund result")
					},
				}
			}
		}

		/// Process a pending balance check.
		fn process_balance_check(api_key: &str) {
			if !BalanceCheckRequested::<T>::get() {
				return;
			}

			log::info!("[stripe-bridge] processing balance check");
			match stripe_client::get_balance(api_key) {
				Ok((code, body)) if code >= 200 && code < 300 => {
					let val: serde_json::Value = match serde_json::from_slice(&body) {
						Ok(v) => v,
						Err(_) => return,
					};

					let (avail_amount, avail_currency) = val
						.get("available")
						.and_then(|a| a.as_array())
						.and_then(|arr| arr.first())
						.map(|entry| {
							let amt = entry.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
							let cur = entry
								.get("currency")
								.and_then(|v| v.as_str())
								.unwrap_or("")
								.as_bytes()
								.to_vec();
							(amt, cur)
						})
						.unwrap_or((0, b"".to_vec()));

					let (pend_amount, pend_currency) = val
						.get("pending")
						.and_then(|a| a.as_array())
						.and_then(|arr| arr.first())
						.map(|entry| {
							let amt = entry.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
							let cur = entry
								.get("currency")
								.and_then(|v| v.as_str())
								.unwrap_or("")
								.as_bytes()
								.to_vec();
							(amt, cur)
						})
						.unwrap_or((0, b"".to_vec()));

					Self::send_balance_result(
						avail_amount,
						avail_currency,
						pend_amount,
						pend_currency,
					);
				},
				_ => {
					log::warn!("[stripe-bridge] balance check failed");
				},
			}
		}

		/// Submit an unsigned transaction with a balance result.
		fn send_balance_result(
			available_amount: i64,
			available_currency: Vec<u8>,
			pending_amount: i64,
			pending_currency: Vec<u8>,
		) {
			let signer = Signer::<T, T::AuthorityId>::any_account();
			if let Some((_, res)) = signer.send_unsigned_transaction(
				|account| BalanceResultPayload {
					available_amount,
					available_currency: available_currency.clone(),
					pending_amount,
					pending_currency: pending_currency.clone(),
					public: account.public.clone(),
				},
				|payload, signature| Call::submit_balance_result { payload, signature },
			) {
				match res {
					Ok(_) => log::info!("[stripe-bridge] submitted balance result"),
					Err(()) => {
						log::error!("[stripe-bridge] failed to submit balance result")
					},
				}
			}
		}

		// -------------------------------------------------------------------
		// Query helpers
		// -------------------------------------------------------------------

		pub fn query_pending_payment(index: u64) -> Option<StripePaymentRequest<T::AccountId>> {
			Self::pending_payments(index)
		}

		pub fn query_processed_payment(index: u64) -> Option<StripePaymentRecord> {
			Self::processed_payments(index)
		}

		pub fn query_pending_refund(index: u64) -> Option<StripeRefundRequest> {
			Self::pending_refunds(index)
		}

		pub fn query_processed_refund(index: u64) -> Option<StripeRefundRecord> {
			Self::processed_refunds(index)
		}

		pub fn query_bridge_transfer(bridge_id: u64) -> Option<BridgeTransfer<T::AccountId>> {
			Self::bridge_transfers(bridge_id)
		}

		pub fn query_last_balance() -> Option<StripeBalanceInfo> {
			Self::last_balance()
		}

		fn bridge_transfer_escrow_reference(bridge_id: u64) -> Vec<u8> {
			format!("bridge-transfer-{}", bridge_id).into_bytes()
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn canonical_outbound_idempotency_key(bridge_id: u64) -> Vec<u8> {
			format!("stripe-bridge-outbound-{}", bridge_id).into_bytes()
		}

		fn outbound_transfer_in_flight_storage_key(bridge_id: u64) -> Vec<u8> {
			let mut key = OUTBOUND_TRANSFER_IN_FLIGHT_PREFIX.to_vec();
			key.extend_from_slice(b"::");
			key.extend_from_slice(&bridge_id.encode());
			key
		}

		fn has_fresh_outbound_transfer_in_flight_marker(bridge_id: u64) -> bool {
			let storage_key = Self::outbound_transfer_in_flight_storage_key(bridge_id);
			let Some(stored_marker) = sp_io::offchain::local_storage_get(
				sp_core::offchain::StorageKind::PERSISTENT,
				&storage_key,
			) else {
				return false;
			};

			let Some(stored_at_ms) = u64::decode(&mut &stored_marker[..]).ok() else {
				return false;
			};
			let now_ms = sp_io::offchain::timestamp().unix_millis();
			Self::is_outbound_transfer_in_flight_marker_fresh(stored_at_ms, now_ms)
		}

		fn mark_outbound_transfer_in_flight(bridge_id: u64) {
			let storage_key = Self::outbound_transfer_in_flight_storage_key(bridge_id);
			let now_ms = sp_io::offchain::timestamp().unix_millis();
			sp_io::offchain::local_storage_set(
				sp_core::offchain::StorageKind::PERSISTENT,
				&storage_key,
				&now_ms.encode(),
			);
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn is_outbound_transfer_in_flight_marker_fresh(
			stored_at_ms: u64,
			now_ms: u64,
		) -> bool {
			now_ms.saturating_sub(stored_at_ms) < OUTBOUND_TRANSFER_IN_FLIGHT_TTL_MS
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn build_unsigned_provides_key(prefix: &[u8], logical_id: u64) -> Vec<u8> {
			let mut key = prefix.to_vec();
			key.extend_from_slice(&logical_id.encode());
			key
		}

		fn create_outbound_bridge_transfer(
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			retry_of: Option<u64>,
		) -> Result<u64, DispatchError> {
			let bridge_id = Self::create_bridge_transfer_with_retry_of(
				owner.clone(),
				amount,
				currency.clone(),
				BridgeTransferDirection::ToStripe,
				retry_of,
			)?;
			let escrow_reference = Self::bridge_transfer_escrow_reference(bridge_id);

			remuneration::Pallet::<T>::bridge_reserve_funds(
				escrow_reference.clone(),
				owner,
				amount.saturated_into(),
			)?;
			Self::attach_bridge_transfer_escrow_reference(bridge_id, escrow_reference)?;
			Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::FundsReserved)?;

			Ok(bridge_id)
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn create_bridge_transfer(
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			direction: BridgeTransferDirection,
		) -> Result<u64, DispatchError> {
			Self::create_bridge_transfer_with_retry_of(owner, amount, currency, direction, None)
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn create_bridge_transfer_with_retry_of(
			owner: T::AccountId,
			amount: u64,
			currency: Vec<u8>,
			direction: BridgeTransferDirection,
			retry_of: Option<u64>,
		) -> Result<u64, DispatchError> {
			let bridge_id = NextBridgeTransferId::<T>::get();
			let next_bridge_id =
				bridge_id.checked_add(1).ok_or(sp_runtime::ArithmeticError::Overflow)?;
			let bounded_currency = Self::bound_bridge_field::<8>(currency.clone())?;

			let transfer = BridgeTransfer {
				owner: owner.clone(),
				amount,
				currency: bounded_currency,
				direction,
				status: BridgeTransferStatus::Requested,
				retry_of,
				stripe_object_id: None,
				external_reference: None,
				escrow_reference: None,
				last_error: None,
			};

			BridgeTransfers::<T>::insert(bridge_id, transfer);
			NextBridgeTransferId::<T>::put(next_bridge_id);

			Self::deposit_event(Event::BridgeTransferCreated {
				bridge_id,
				owner,
				amount,
				currency,
				direction,
				status: BridgeTransferStatus::Requested,
			});

			Ok(bridge_id)
		}

		fn has_retry_descendant(original_bridge_id: u64) -> bool {
			BridgeTransfers::<T>::iter()
				.any(|(_, transfer)| transfer.retry_of == Some(original_bridge_id))
		}

		fn is_failed_terminal_bridge_status(status: BridgeTransferStatus) -> bool {
			matches!(status, BridgeTransferStatus::Failed | BridgeTransferStatus::Reverted)
		}

		fn is_successful_terminal_bridge_status(transfer: &BridgeTransfer<T::AccountId>) -> bool {
			transfer.status == BridgeTransferStatus::Finalized
				&& matches!(
					transfer.direction,
					BridgeTransferDirection::ToStripe | BridgeTransferDirection::FromStripe
				) && (transfer.direction == BridgeTransferDirection::FromStripe
				|| transfer.stripe_object_id.is_some())
		}

		fn is_retryable_outbound_transfer(
			bridge_id: u64,
			transfer: &BridgeTransfer<T::AccountId>,
		) -> bool {
			transfer.direction == BridgeTransferDirection::ToStripe
				&& Self::is_failed_terminal_bridge_status(transfer.status)
				&& !Self::has_retry_descendant(bridge_id)
		}

		fn is_force_revertable_outbound_transfer(transfer: &BridgeTransfer<T::AccountId>) -> bool {
			transfer.direction == BridgeTransferDirection::ToStripe
				&& matches!(
					transfer.status,
					BridgeTransferStatus::FundsReserved | BridgeTransferStatus::SubmittedToStripe
				)
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn update_bridge_transfer_status(
			bridge_id: u64,
			new_status: BridgeTransferStatus,
		) -> DispatchResult {
			BridgeTransfers::<T>::try_mutate(bridge_id, |maybe_transfer| -> DispatchResult {
				let transfer = maybe_transfer.as_mut().ok_or(Error::<T>::BridgeTransferNotFound)?;
				let old_status = transfer.status;

				ensure!(
					Self::is_valid_bridge_status_transition(
						transfer.direction,
						old_status,
						new_status,
					),
					Error::<T>::InvalidBridgeTransferStatusTransition
				);

				transfer.status = new_status;
				Self::deposit_event(Event::BridgeTransferStatusUpdated {
					bridge_id,
					old_status,
					new_status,
				});

				Ok(())
			})
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn attach_bridge_transfer_stripe_object_id(
			bridge_id: u64,
			stripe_object_id: Vec<u8>,
		) -> DispatchResult {
			let bounded_stripe_object_id = Self::bound_bridge_field::<128>(stripe_object_id)?;

			BridgeTransfers::<T>::try_mutate(bridge_id, |maybe_transfer| -> DispatchResult {
				let transfer = maybe_transfer.as_mut().ok_or(Error::<T>::BridgeTransferNotFound)?;
				transfer.stripe_object_id = Some(bounded_stripe_object_id);
				Ok(())
			})
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn attach_bridge_transfer_external_reference(
			bridge_id: u64,
			external_reference: Vec<u8>,
		) -> DispatchResult {
			let bounded_external_reference = Self::bound_bridge_field::<128>(external_reference)?;

			BridgeTransfers::<T>::try_mutate(bridge_id, |maybe_transfer| -> DispatchResult {
				let transfer = maybe_transfer.as_mut().ok_or(Error::<T>::BridgeTransferNotFound)?;
				transfer.external_reference = Some(bounded_external_reference);
				Ok(())
			})
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn attach_bridge_transfer_escrow_reference(
			bridge_id: u64,
			escrow_reference: Vec<u8>,
		) -> DispatchResult {
			let bounded_escrow_reference = Self::bound_bridge_field::<128>(escrow_reference)?;

			BridgeTransfers::<T>::try_mutate(bridge_id, |maybe_transfer| -> DispatchResult {
				let transfer = maybe_transfer.as_mut().ok_or(Error::<T>::BridgeTransferNotFound)?;
				transfer.escrow_reference = Some(bounded_escrow_reference);
				Ok(())
			})
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub(crate) fn attach_bridge_transfer_last_error(
			bridge_id: u64,
			last_error: Vec<u8>,
		) -> DispatchResult {
			let bounded_last_error = Self::bound_bridge_field::<256>(last_error)?;

			BridgeTransfers::<T>::try_mutate(bridge_id, |maybe_transfer| -> DispatchResult {
				let transfer = maybe_transfer.as_mut().ok_or(Error::<T>::BridgeTransferNotFound)?;
				transfer.last_error = Some(bounded_last_error);
				Ok(())
			})
		}

		#[cfg_attr(not(test), allow(dead_code))]
		fn is_valid_bridge_status_transition(
			direction: BridgeTransferDirection,
			current_status: BridgeTransferStatus,
			new_status: BridgeTransferStatus,
		) -> bool {
			matches!(
				(direction, current_status, new_status),
				(
					BridgeTransferDirection::ToStripe,
					BridgeTransferStatus::Requested,
					BridgeTransferStatus::FundsReserved
				) | (
					BridgeTransferDirection::ToStripe,
					BridgeTransferStatus::FundsReserved,
					BridgeTransferStatus::SubmittedToStripe | BridgeTransferStatus::Reverted
				) | (
					BridgeTransferDirection::ToStripe,
					BridgeTransferStatus::SubmittedToStripe,
					BridgeTransferStatus::Succeeded | BridgeTransferStatus::Failed
				) | (
					BridgeTransferDirection::ToStripe,
					BridgeTransferStatus::Succeeded,
					BridgeTransferStatus::Finalized
				) | (
					BridgeTransferDirection::ToStripe,
					BridgeTransferStatus::Failed,
					BridgeTransferStatus::Reverted
				) | (
					BridgeTransferDirection::FromStripe,
					BridgeTransferStatus::Requested,
					BridgeTransferStatus::CreditedOnChain
				) | (
					BridgeTransferDirection::FromStripe,
					BridgeTransferStatus::CreditedOnChain,
					BridgeTransferStatus::Finalized
				)
			)
		}

		#[cfg_attr(not(test), allow(dead_code))]
		fn bound_bridge_field<const N: u32>(
			value: Vec<u8>,
		) -> Result<BoundedVec<u8, ConstU32<N>>, DispatchError> {
			value.try_into().map_err(|_| Error::<T>::BridgeFieldTooLong.into())
		}

		fn handle_outbound_transfer_result(
			bridge_id: u64,
			success: bool,
			stripe_object_id: Vec<u8>,
			stripe_status: Vec<u8>,
			error_message: Vec<u8>,
		) -> DispatchResult {
			let transfer =
				BridgeTransfers::<T>::get(bridge_id).ok_or(Error::<T>::BridgeTransferNotFound)?;
			ensure!(
				transfer.direction == BridgeTransferDirection::ToStripe,
				Error::<T>::InvalidBridgeTransferDirection
			);
			ensure!(
				transfer.status == BridgeTransferStatus::FundsReserved,
				Error::<T>::InvalidBridgeTransferStatusTransition
			);

			let escrow_reference = Self::bridge_transfer_escrow_reference(bridge_id);
			Self::update_bridge_transfer_status(
				bridge_id,
				BridgeTransferStatus::SubmittedToStripe,
			)?;

			if success {
				if !stripe_object_id.is_empty() {
					Self::attach_bridge_transfer_stripe_object_id(
						bridge_id,
						stripe_object_id.clone(),
					)?;
				}
				Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Succeeded)?;
				remuneration::Pallet::<T>::bridge_finalize_outbound(escrow_reference)?;
				Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Finalized)?;
				Self::deposit_event(Event::OutboundTransferToStripeSucceeded {
					bridge_id,
					stripe_object_id,
				});
				return Ok(());
			}

			let failure_reason =
				if !error_message.is_empty() { error_message } else { stripe_status };
			if !failure_reason.is_empty() {
				Self::attach_bridge_transfer_last_error(bridge_id, failure_reason.clone())?;
			}
			Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Failed)?;
			remuneration::Pallet::<T>::bridge_release_funds(escrow_reference)?;
			Self::update_bridge_transfer_status(bridge_id, BridgeTransferStatus::Reverted)?;
			Self::deposit_event(Event::OutboundTransferToStripeFailed {
				bridge_id,
				reason: failure_reason,
			});
			Ok(())
		}
	}
}
