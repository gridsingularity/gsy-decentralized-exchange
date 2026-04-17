use crate::{
	mock::*,
	pallet::{
		BalanceCheckRequested, BridgeTransferDirection, BridgeTransferStatus, BridgeTransfers,
		LastBalance, NextBridgeTransferId, NextPaymentIndex, NextRefundIndex, PendingPayments,
		PendingRefunds, ProcessedPayments, ProcessedRefunds, StripeEnabled, StripePaymentRecord,
	},
	OUTBOUND_TRANSFER_IN_FLIGHT_TTL_MS, STRIPE_API_KEY_STORAGE,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use gsy_primitives::v0::AccountId;
use remuneration::pallet::BridgeEscrowStatus;
use sp_core::sr25519;

fn alice() -> AccountId {
	AccountId::from(sr25519::Public::from_raw([1u8; 32]))
}
fn bob() -> AccountId {
	AccountId::from(sr25519::Public::from_raw([2u8; 32]))
}
fn charlie() -> AccountId {
	AccountId::from(sr25519::Public::from_raw([3u8; 32]))
}

/// Helper: register `who` as custodian in the remuneration pallet.
fn set_custodian(who: AccountId) {
	assert_ok!(Remuneration::update_custodian(RuntimeOrigin::signed(who.clone()), who,));
}

fn set_remuneration_balance(who: AccountId, amount: u128) {
	assert_ok!(Remuneration::set_balance(RuntimeOrigin::signed(alice()), who, amount,));
}

fn bridge_reference(bridge_id: u64) -> Vec<u8> {
	format!("bridge-transfer-{bridge_id}").into_bytes()
}

// ===========================================================================
//  Section 1: On-chain extrinsic tests
// ===========================================================================

// Verifies that the remuneration custodian can toggle the Stripe bridge on and off.
#[test]
fn set_stripe_enabled_works_for_custodian() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert!(StripeEnabled::<Test>::get());

		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), false));
		assert!(!StripeEnabled::<Test>::get());
	});
}

// Verifies that non-custodian accounts cannot toggle the Stripe bridge.
#[test]
fn set_stripe_enabled_fails_for_non_custodian() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_noop!(
			StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(bob()), true),
			crate::Error::<Test>::NotCustodian
		);
	});
}

// Verifies that unsigned provides keys include the logical record id for deduplication safety.
#[test]
fn unsigned_provides_keys_are_unique_per_logical_id() {
	new_test_ext().execute_with(|| {
		let payment_key = StripeBridge::build_unsigned_provides_key(b"submit_payment_result", 7);
		let refund_key = StripeBridge::build_unsigned_provides_key(b"submit_refund_result", 7);
		let outbound_key =
			StripeBridge::build_unsigned_provides_key(b"submit_outbound_transfer_result", 7);

		assert_eq!(payment_key, [b"submit_payment_result".as_slice(), &7u64.encode()].concat());
		assert_eq!(refund_key, [b"submit_refund_result".as_slice(), &7u64.encode()].concat());
		assert_eq!(
			outbound_key,
			[b"submit_outbound_transfer_result".as_slice(), &7u64.encode()].concat()
		);
		assert_ne!(
			StripeBridge::build_unsigned_provides_key(b"submit_payment_result", 1),
			StripeBridge::build_unsigned_provides_key(b"submit_payment_result", 2)
		);
		assert_ne!(payment_key, refund_key);
		assert_ne!(payment_key, outbound_key);
	});
}

// Verifies that canonical outbound Stripe idempotency keys are stable per bridge id.
#[test]
fn canonical_outbound_idempotency_keys_are_stable_and_unique() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			StripeBridge::canonical_outbound_idempotency_key(42),
			b"stripe-bridge-outbound-42".to_vec()
		);
		assert_eq!(
			StripeBridge::canonical_outbound_idempotency_key(42),
			StripeBridge::canonical_outbound_idempotency_key(42)
		);
		assert_ne!(
			StripeBridge::canonical_outbound_idempotency_key(41),
			StripeBridge::canonical_outbound_idempotency_key(42)
		);
	});
}

// Verifies that a valid payment request is queued with the expected stored fields.
#[test]
fn queue_stripe_payment_works() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_ok!(StripeBridge::queue_stripe_payment(
			RuntimeOrigin::signed(alice()),
			bob(),
			1000,
			b"chf".to_vec(),
		));

		assert_eq!(NextPaymentIndex::<Test>::get(), 1);
		let req = PendingPayments::<Test>::get(0).expect("should exist");
		assert_eq!(req.receiver, bob());
		assert_eq!(req.amount, 1000);
		assert_eq!(req.currency.as_slice(), b"chf");
	});
}

// Verifies that each queued payment consumes a fresh auto-incremented payment index.
#[test]
fn queue_stripe_payment_increments_index() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		for i in 0..3u64 {
			assert_ok!(StripeBridge::queue_stripe_payment(
				RuntimeOrigin::signed(alice()),
				bob(),
				(i + 1) * 100,
				b"usd".to_vec(),
			));
		}
		assert_eq!(NextPaymentIndex::<Test>::get(), 3);
		assert!(PendingPayments::<Test>::get(0).is_some());
		assert!(PendingPayments::<Test>::get(1).is_some());
		assert!(PendingPayments::<Test>::get(2).is_some());
	});
}

// Verifies that payments cannot be queued while the Stripe bridge is disabled.
#[test]
fn queue_stripe_payment_fails_when_disabled() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_noop!(
			StripeBridge::queue_stripe_payment(
				RuntimeOrigin::signed(alice()),
				bob(),
				1000,
				b"chf".to_vec(),
			),
			crate::Error::<Test>::StripeNotEnabled
		);
	});
}

// Verifies that only the remuneration custodian can queue Stripe payments.
#[test]
fn queue_stripe_payment_fails_for_non_custodian() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::queue_stripe_payment(
				RuntimeOrigin::signed(bob()),
				charlie(),
				1000,
				b"chf".to_vec(),
			),
			crate::Error::<Test>::NotCustodian
		);
	});
}

// Verifies that oversized currency codes are rejected before being stored.
#[test]
fn queue_stripe_payment_rejects_long_currency() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::queue_stripe_payment(
				RuntimeOrigin::signed(alice()),
				bob(),
				1000,
				b"very_long_currency_code".to_vec(),
			),
			crate::Error::<Test>::CurrencyTooLong
		);
	});
}

// Verifies that an offchain-submitted payment result clears the pending item and stores the processed record.
#[test]
fn submit_payment_result_stores_record() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::queue_stripe_payment(
			RuntimeOrigin::signed(alice()),
			bob(),
			1000,
			b"chf".to_vec(),
		));

		let payload = crate::pallet::PaymentResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			payment_index: 0,
			stripe_payment_id: b"pi_test_123".to_vec(),
			status: b"succeeded".to_vec(),
			gross_amount: 1000,
			stripe_fee: 29,
			net_amount: 971,
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_payment_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert!(PendingPayments::<Test>::get(0).is_none());
		let record = ProcessedPayments::<Test>::get(0).expect("should exist");
		assert_eq!(record.stripe_payment_id.as_slice(), b"pi_test_123");
		assert_eq!(record.status.as_slice(), b"succeeded");
		assert_eq!(record.gross_amount, 1000);
		assert_eq!(record.stripe_fee, 29);
		assert_eq!(record.net_amount, 971);
	});
}

// Verifies that a refund can be queued from a previously processed Stripe payment.
#[test]
fn queue_stripe_refund_works() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		let record = StripePaymentRecord {
			stripe_payment_id: b"pi_test_456".to_vec().try_into().unwrap(),
			status: b"succeeded".to_vec().try_into().unwrap(),
			gross_amount: 2000,
			stripe_fee: 58,
			net_amount: 1942,
		};
		ProcessedPayments::<Test>::insert(0u64, record);

		assert_ok!(StripeBridge::queue_stripe_refund(RuntimeOrigin::signed(alice()), 0,));

		let refund = PendingRefunds::<Test>::get(0).expect("should exist");
		assert_eq!(refund.payment_index, 0);
		assert_eq!(refund.stripe_payment_id.as_slice(), b"pi_test_456");
		assert_eq!(NextRefundIndex::<Test>::get(), 1);
	});
}

// Verifies that refund requests fail when the referenced processed payment does not exist.
#[test]
fn queue_stripe_refund_fails_without_processed_payment() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::queue_stripe_refund(RuntimeOrigin::signed(alice()), 999),
			crate::Error::<Test>::PaymentNotFound
		);
	});
}

// Verifies that an offchain-submitted refund result is persisted in processed refunds.
#[test]
fn submit_refund_result_stores_record() {
	new_test_ext().execute_with(|| {
		let payload = crate::pallet::RefundResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			refund_index: 0,
			refund_id: b"re_test_789".to_vec(),
			status: b"succeeded".to_vec(),
			amount: 2000,
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};

		assert_ok!(StripeBridge::submit_refund_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		let record = ProcessedRefunds::<Test>::get(0).expect("should exist");
		assert_eq!(record.refund_id.as_slice(), b"re_test_789");
		assert_eq!(record.status.as_slice(), b"succeeded");
		assert_eq!(record.amount, 2000);
	});
}

// Verifies that requesting a balance check sets the on-chain flag consumed by the offchain worker.
#[test]
fn request_balance_check_sets_flag() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert!(!BalanceCheckRequested::<Test>::get());
		assert_ok!(StripeBridge::request_balance_check(RuntimeOrigin::signed(alice())));
		assert!(BalanceCheckRequested::<Test>::get());
	});
}

// Verifies that a canonical bridge transfer can be created and receives a fresh id.
#[test]
fn create_bridge_transfer_works() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			2500,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("bridge transfer should be created");

		assert_eq!(bridge_id, 0);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert!(BridgeTransfers::<Test>::contains_key(bridge_id));
	});
}

// Verifies the stored canonical transfer fields and default lifecycle status.
#[test]
fn create_bridge_transfer_stores_expected_defaults() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			2500,
			b"usd".to_vec(),
			BridgeTransferDirection::FromStripe,
		)
		.expect("bridge transfer should be created");

		let transfer = BridgeTransfers::<Test>::get(bridge_id).expect("transfer should exist");
		assert_eq!(transfer.owner, bob());
		assert_eq!(transfer.amount, 2500);
		assert_eq!(transfer.currency.as_slice(), b"usd");
		assert_eq!(transfer.direction, BridgeTransferDirection::FromStripe);
		assert_eq!(transfer.status, BridgeTransferStatus::Requested);
		assert!(transfer.stripe_object_id.is_none());
		assert!(transfer.external_reference.is_none());
		assert!(transfer.escrow_reference.is_none());
		assert!(transfer.last_error.is_none());
	});
}

// Verifies that the canonical transfer query helper returns the stored record.
#[test]
fn query_bridge_transfer_works() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			alice(),
			900,
			b"eur".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("bridge transfer should be created");

		assert_ok!(StripeBridge::attach_bridge_transfer_stripe_object_id(
			bridge_id,
			b"pi_bridge_query".to_vec(),
		));
		assert_ok!(StripeBridge::attach_bridge_transfer_external_reference(
			bridge_id,
			b"stripe-ext-42".to_vec(),
		));
		assert_ok!(StripeBridge::attach_bridge_transfer_escrow_reference(
			bridge_id,
			b"escrow-42".to_vec(),
		));
		assert_ok!(StripeBridge::attach_bridge_transfer_last_error(
			bridge_id,
			b"temporary network timeout".to_vec(),
		));

		let transfer =
			StripeBridge::query_bridge_transfer(bridge_id).expect("transfer should be queryable");
		assert_eq!(transfer.owner, alice());
		assert_eq!(
			transfer
				.stripe_object_id
				.as_ref()
				.expect("stripe object id should be set")
				.as_slice(),
			b"pi_bridge_query"
		);
		assert_eq!(
			transfer
				.external_reference
				.as_ref()
				.expect("external reference should be set")
				.as_slice(),
			b"stripe-ext-42"
		);
		assert_eq!(
			transfer
				.escrow_reference
				.as_ref()
				.expect("escrow reference should be set")
				.as_slice(),
			b"escrow-42"
		);
		assert_eq!(
			transfer.last_error.as_ref().expect("last error should be set").as_slice(),
			b"temporary network timeout"
		);
	});
}

// Verifies that the intended outbound canonical lifecycle transitions remain valid.
#[test]
fn update_bridge_transfer_status_accepts_valid_outbound_sequence() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("bridge transfer should be created");

		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::SubmittedToStripe,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::Succeeded,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::Finalized,
		));

		let transfer =
			StripeBridge::query_bridge_transfer(bridge_id).expect("transfer should exist");
		assert_eq!(transfer.status, BridgeTransferStatus::Finalized);
	});
}

// Verifies that the intended inbound canonical lifecycle transitions remain valid.
#[test]
fn update_bridge_transfer_status_accepts_valid_inbound_sequence() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::FromStripe,
		)
		.expect("bridge transfer should be created");

		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::CreditedOnChain,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::Finalized,
		));

		let transfer =
			StripeBridge::query_bridge_transfer(bridge_id).expect("transfer should exist");
		assert_eq!(transfer.status, BridgeTransferStatus::Finalized);
	});
}

// Verifies that the intended outbound failure lifecycle transitions remain valid.
#[test]
fn update_bridge_transfer_status_accepts_valid_outbound_failure_sequence() {
	new_test_ext().execute_with(|| {
		let bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("bridge transfer should be created");

		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::SubmittedToStripe,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::Failed,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			bridge_id,
			BridgeTransferStatus::Reverted,
		));

		let transfer =
			StripeBridge::query_bridge_transfer(bridge_id).expect("transfer should exist");
		assert_eq!(transfer.status, BridgeTransferStatus::Reverted);
	});
}

// Verifies that dangerous canonical lifecycle jumps are rejected explicitly.
#[test]
fn update_bridge_transfer_status_rejects_dangerous_transitions() {
	new_test_ext().execute_with(|| {
		let requested_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("requested transfer should be created");
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				requested_bridge_id,
				BridgeTransferStatus::Finalized,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let reserved_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("reserved transfer should be created");
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			reserved_bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				reserved_bridge_id,
				BridgeTransferStatus::Finalized,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let succeeded_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("succeeded transfer should be created");
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			succeeded_bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			succeeded_bridge_id,
			BridgeTransferStatus::SubmittedToStripe,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			succeeded_bridge_id,
			BridgeTransferStatus::Succeeded,
		));
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				succeeded_bridge_id,
				BridgeTransferStatus::Reverted,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let failed_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("failed transfer should be created");
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			failed_bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			failed_bridge_id,
			BridgeTransferStatus::SubmittedToStripe,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			failed_bridge_id,
			BridgeTransferStatus::Failed,
		));
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				failed_bridge_id,
				BridgeTransferStatus::Finalized,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let reverted_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::ToStripe,
		)
		.expect("reverted transfer should be created");
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			reverted_bridge_id,
			BridgeTransferStatus::FundsReserved,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			reverted_bridge_id,
			BridgeTransferStatus::SubmittedToStripe,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			reverted_bridge_id,
			BridgeTransferStatus::Failed,
		));
		assert_ok!(StripeBridge::update_bridge_transfer_status(
			reverted_bridge_id,
			BridgeTransferStatus::Reverted,
		));
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				reverted_bridge_id,
				BridgeTransferStatus::Finalized,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let inbound_bridge_id = StripeBridge::create_bridge_transfer(
			bob(),
			1000,
			b"chf".to_vec(),
			BridgeTransferDirection::FromStripe,
		)
		.expect("inbound transfer should be created");
		assert_noop!(
			StripeBridge::update_bridge_transfer_status(
				inbound_bridge_id,
				BridgeTransferStatus::AwaitingConfirmation,
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);
	});
}

// Verifies that oversized canonical transfer fields are rejected by bounded storage.
#[test]
fn create_bridge_transfer_rejects_oversized_bounded_field() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			StripeBridge::create_bridge_transfer(
				bob(),
				1000,
				b"currency-too-long".to_vec(),
				BridgeTransferDirection::ToStripe,
			),
			crate::Error::<Test>::BridgeFieldTooLong
		);
	});
}

// Verifies that requesting an outbound transfer creates a canonical transfer and reserves funds.
#[test]
fn request_transfer_to_stripe_creates_transfer_and_reserves_funds() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		let escrow_reference = bridge_reference(0);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert_eq!(transfer.owner, bob());
		assert_eq!(transfer.amount, 400);
		assert_eq!(transfer.direction, BridgeTransferDirection::ToStripe);
		assert_eq!(transfer.status, BridgeTransferStatus::FundsReserved);
		assert_eq!(
			transfer
				.escrow_reference
				.as_ref()
				.expect("escrow reference should be set")
				.as_slice(),
			escrow_reference.as_slice()
		);
		assert!(transfer.external_reference.is_none());
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 400);
		let escrow =
			Remuneration::query_bridge_escrow(&escrow_reference).expect("escrow should exist");
		assert_eq!(escrow.owner, bob());
		assert_eq!(escrow.amount, 400);
		assert_eq!(escrow.status, BridgeEscrowStatus::Active);
	});
}

// Verifies that outbound transfer requests fail cleanly when remuneration reserve fails.
#[test]
fn request_transfer_to_stripe_fails_when_remuneration_reserve_fails() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::request_transfer_to_stripe(
				RuntimeOrigin::signed(alice()),
				bob(),
				400,
				b"chf".to_vec(),
			),
			remuneration::Error::<Test>::BridgeInsufficientBalance
		);

		assert_eq!(NextBridgeTransferId::<Test>::get(), 0);
		assert!(StripeBridge::query_bridge_transfer(0).is_none());
		assert_eq!(Remuneration::balances(bob()), 100);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
	});
}

// Verifies that a successful outbound transfer result finalizes remuneration escrow.
#[test]
fn submit_outbound_transfer_result_success_finalizes_escrow() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: true,
			stripe_object_id: b"pi_outbound_001".to_vec(),
			stripe_status: b"succeeded".to_vec(),
			error_message: Vec::new(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};

		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		let escrow_reference = bridge_reference(0);
		assert_eq!(transfer.status, BridgeTransferStatus::Finalized);
		assert_eq!(
			transfer
				.stripe_object_id
				.as_ref()
				.expect("stripe object id should be set")
				.as_slice(),
			b"pi_outbound_001"
		);
		assert!(transfer.last_error.is_none());
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
		assert_eq!(
			Remuneration::query_bridge_escrow(&escrow_reference)
				.expect("escrow should exist")
				.status,
			BridgeEscrowStatus::Finalized
		);
	});
}

// Verifies that a failed outbound transfer result releases remuneration escrow back to the user.
#[test]
fn submit_outbound_transfer_result_failure_releases_escrow() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: false,
			stripe_object_id: Vec::new(),
			stripe_status: b"failed".to_vec(),
			error_message: b"card_declined".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};

		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		let escrow_reference = bridge_reference(0);
		assert_eq!(transfer.status, BridgeTransferStatus::Reverted);
		assert_eq!(
			transfer.last_error.as_ref().expect("last error should be set").as_slice(),
			b"card_declined"
		);
		assert!(transfer.stripe_object_id.is_none());
		assert_eq!(Remuneration::balances(bob()), 1000);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
		assert_eq!(
			Remuneration::query_bridge_escrow(&escrow_reference)
				.expect("escrow should exist")
				.status,
			BridgeEscrowStatus::Released
		);
	});
}

// Verifies that repeated outbound settlement submissions cannot double-finalize remuneration.
#[test]
fn submit_outbound_transfer_result_rejects_repeated_settlement() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: true,
			stripe_object_id: b"pi_outbound_repeat".to_vec(),
			stripe_status: b"succeeded".to_vec(),
			error_message: Vec::new(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};

		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			payload.clone(),
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));
		assert_noop!(
			StripeBridge::submit_outbound_transfer_result(
				RuntimeOrigin::none(),
				payload,
				sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
					[0u8; 64]
				)),
			),
			crate::Error::<Test>::InvalidBridgeTransferStatusTransition
		);

		let escrow_reference = bridge_reference(0);
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
		assert_eq!(
			Remuneration::query_bridge_escrow(&escrow_reference)
				.expect("escrow should exist")
				.status,
			BridgeEscrowStatus::Finalized
		);
	});
}

// Verifies that retrying a reverted outbound transfer creates a fresh linked transfer and reserves funds again.
#[test]
fn retry_transfer_to_stripe_creates_fresh_linked_transfer() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let failure_payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: false,
			stripe_object_id: Vec::new(),
			stripe_status: b"failed".to_vec(),
			error_message: b"card_declined".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			failure_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert_ok!(StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0,));

		let original =
			StripeBridge::query_bridge_transfer(0).expect("original transfer should exist");
		let retry = StripeBridge::query_bridge_transfer(1).expect("retry transfer should exist");
		assert_eq!(NextBridgeTransferId::<Test>::get(), 2);
		assert_eq!(original.status, BridgeTransferStatus::Reverted);
		assert_eq!(retry.owner, bob());
		assert_eq!(retry.amount, 400);
		assert_eq!(retry.direction, BridgeTransferDirection::ToStripe);
		assert_eq!(retry.status, BridgeTransferStatus::FundsReserved);
		assert_eq!(retry.retry_of, Some(0));
		assert_eq!(
			retry
				.escrow_reference
				.as_ref()
				.expect("retry escrow reference should exist")
				.as_slice(),
			bridge_reference(1).as_slice()
		);
		assert!(retry.external_reference.is_none());
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 400);
		assert_eq!(
			Remuneration::query_bridge_escrow(&bridge_reference(1))
				.expect("retry escrow should exist")
				.status,
			BridgeEscrowStatus::Active
		);
	});
}

// Verifies that retrying the same failed outbound transfer twice does not create duplicate reserves.
#[test]
fn retry_transfer_to_stripe_rejects_repeated_retry_of_same_transfer() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let failure_payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: false,
			stripe_object_id: Vec::new(),
			stripe_status: b"failed".to_vec(),
			error_message: b"network_error".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			failure_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert_ok!(StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0,));
		assert_noop!(
			StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0),
			crate::Error::<Test>::OutboundTransferNotRetryable
		);

		assert_eq!(NextBridgeTransferId::<Test>::get(), 2);
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 400);
	});
}

// Verifies that retry is rejected for a successfully finalized outbound transfer.
#[test]
fn retry_transfer_to_stripe_rejects_successfully_finalized_transfer() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let success_payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: true,
			stripe_object_id: b"pi_success".to_vec(),
			stripe_status: b"succeeded".to_vec(),
			error_message: Vec::new(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			success_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert_noop!(
			StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0),
			crate::Error::<Test>::OutboundTransferNotRetryable
		);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert_eq!(Remuneration::balances(bob()), 600);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
	});
}

// Verifies that retry is rejected for non-outbound canonical transfers.
#[test]
fn retry_transfer_to_stripe_rejects_non_outbound_transfer() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::confirm_transfer_from_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			250,
			b"chf".to_vec(),
			b"stripe-ext-retry".to_vec(),
			b"pi_inbound_retry".to_vec(),
		));

		assert_noop!(
			StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0),
			crate::Error::<Test>::InvalidBridgeTransferDirection
		);
	});
}

// Verifies that retry is rejected when remuneration cannot reserve funds again.
#[test]
fn retry_transfer_to_stripe_rejects_when_reserve_fails() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let failure_payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: false,
			stripe_object_id: Vec::new(),
			stripe_status: b"failed".to_vec(),
			error_message: b"declined".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			failure_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));
		set_remuneration_balance(bob(), 100);

		assert_noop!(
			StripeBridge::retry_transfer_to_stripe(RuntimeOrigin::signed(alice()), 0),
			remuneration::Error::<Test>::BridgeInsufficientBalance
		);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert!(StripeBridge::query_bridge_transfer(1).is_none());
		assert_eq!(Remuneration::balances(bob()), 100);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
	});
}

// Verifies that force-reverting a stuck outbound transfer releases escrow and records the reason.
#[test]
fn force_revert_outbound_transfer_releases_reserved_funds() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		assert_ok!(StripeBridge::force_revert_outbound_transfer(
			RuntimeOrigin::signed(alice()),
			0,
			b"manual rollback".to_vec(),
		));

		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		assert_eq!(transfer.status, BridgeTransferStatus::Reverted);
		assert_eq!(
			transfer.last_error.as_ref().expect("admin reason should be stored").as_slice(),
			b"manual rollback"
		);
		assert_eq!(Remuneration::balances(bob()), 1000);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
		assert_eq!(
			Remuneration::query_bridge_escrow(&bridge_reference(0))
				.expect("escrow should exist")
				.status,
			BridgeEscrowStatus::Released
		);
	});
}

// Verifies that force-revert cannot be applied to a successfully finalized outbound transfer.
#[test]
fn force_revert_outbound_transfer_rejects_finalized_success() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		let success_payload = crate::pallet::OutboundTransferResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			bridge_id: 0,
			success: true,
			stripe_object_id: b"pi_force_revert_blocked".to_vec(),
			stripe_status: b"succeeded".to_vec(),
			error_message: Vec::new(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_outbound_transfer_result(
			RuntimeOrigin::none(),
			success_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert_noop!(
			StripeBridge::force_revert_outbound_transfer(
				RuntimeOrigin::signed(alice()),
				0,
				b"too late".to_vec(),
			),
			crate::Error::<Test>::OutboundTransferNotForceRevertable
		);
	});
}

// Verifies that force-revert is rejected for non-outbound canonical transfers.
#[test]
fn force_revert_outbound_transfer_rejects_non_outbound_transfer() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::confirm_transfer_from_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			250,
			b"chf".to_vec(),
			b"stripe-ext-force".to_vec(),
			b"pi_inbound_force".to_vec(),
		));

		assert_noop!(
			StripeBridge::force_revert_outbound_transfer(
				RuntimeOrigin::signed(alice()),
				0,
				b"invalid".to_vec(),
			),
			crate::Error::<Test>::InvalidBridgeTransferDirection
		);
	});
}

// Verifies that repeated force-revert attempts do not release funds twice.
#[test]
fn force_revert_outbound_transfer_rejects_repeated_operation() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));
		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			400,
			b"chf".to_vec(),
		));

		assert_ok!(StripeBridge::force_revert_outbound_transfer(
			RuntimeOrigin::signed(alice()),
			0,
			b"first rollback".to_vec(),
		));
		assert_noop!(
			StripeBridge::force_revert_outbound_transfer(
				RuntimeOrigin::signed(alice()),
				0,
				b"second rollback".to_vec(),
			),
			crate::Error::<Test>::OutboundTransferNotForceRevertable
		);

		assert_eq!(Remuneration::balances(bob()), 1000);
		assert_eq!(Remuneration::query_bridge_reserved(&bob()), 0);
		assert_eq!(
			Remuneration::query_bridge_escrow(&bridge_reference(0))
				.expect("escrow should exist")
				.status,
			BridgeEscrowStatus::Released
		);
	});
}

// Verifies that confirming an inbound transfer creates a canonical inbound transfer and credits remuneration.
#[test]
fn confirm_transfer_from_stripe_creates_transfer_and_credits_balance() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_ok!(StripeBridge::confirm_transfer_from_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			250,
			b"chf".to_vec(),
			b"stripe-ext-001".to_vec(),
			b"pi_inbound_001".to_vec(),
		));

		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert_eq!(transfer.owner, bob());
		assert_eq!(transfer.amount, 250);
		assert_eq!(transfer.currency.as_slice(), b"chf");
		assert_eq!(transfer.direction, BridgeTransferDirection::FromStripe);
		assert_eq!(transfer.status, BridgeTransferStatus::Finalized);
		assert_eq!(
			transfer
				.external_reference
				.as_ref()
				.expect("external reference should be set")
				.as_slice(),
			b"stripe-ext-001"
		);
		assert!(transfer.escrow_reference.is_none());
		assert_eq!(
			transfer
				.stripe_object_id
				.as_ref()
				.expect("stripe object id should be set")
				.as_slice(),
			b"pi_inbound_001"
		);
		assert!(transfer.last_error.is_none());
		assert_eq!(Remuneration::balances(bob()), 350);
	});
}

// Verifies that duplicate inbound Stripe references are rejected and do not create inconsistent transfers.
#[test]
fn confirm_transfer_from_stripe_rejects_duplicate_external_reference() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_ok!(StripeBridge::confirm_transfer_from_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			250,
			b"chf".to_vec(),
			b"stripe-ext-dup".to_vec(),
			b"pi_inbound_dup_1".to_vec(),
		));
		assert_noop!(
			StripeBridge::confirm_transfer_from_stripe(
				RuntimeOrigin::signed(alice()),
				bob(),
				250,
				b"chf".to_vec(),
				b"stripe-ext-dup".to_vec(),
				b"pi_inbound_dup_2".to_vec(),
			),
			crate::Error::<Test>::DuplicateInboundExternalReference
		);

		assert_eq!(Remuneration::balances(bob()), 350);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 1);
		assert!(StripeBridge::query_bridge_transfer(1).is_none());
		let transfer = StripeBridge::query_bridge_transfer(0).expect("transfer should exist");
		assert_eq!(transfer.status, BridgeTransferStatus::Finalized);
		assert_eq!(
			transfer
				.stripe_object_id
				.as_ref()
				.expect("stripe object id should be set")
				.as_slice(),
			b"pi_inbound_dup_1"
		);
	});
}

// Verifies that only the remuneration custodian can confirm inbound Stripe transfers.
#[test]
fn confirm_transfer_from_stripe_rejects_unauthorized_caller() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		set_remuneration_balance(bob(), 100);
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::confirm_transfer_from_stripe(
				RuntimeOrigin::signed(bob()),
				bob(),
				250,
				b"chf".to_vec(),
				b"stripe-ext-auth".to_vec(),
				b"pi_inbound_auth".to_vec(),
			),
			crate::Error::<Test>::NotCustodian
		);

		assert_eq!(Remuneration::balances(bob()), 100);
		assert_eq!(NextBridgeTransferId::<Test>::get(), 0);
		assert!(StripeBridge::query_bridge_transfer(0).is_none());
	});
}

// Verifies that an offchain-submitted balance snapshot clears the request flag and stores the snapshot.
#[test]
fn submit_balance_result_stores_info() {
	new_test_ext().execute_with(|| {
		BalanceCheckRequested::<Test>::put(true);

		let payload = crate::pallet::BalanceResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			available_amount: 50000,
			available_currency: b"chf".to_vec(),
			pending_amount: 10000,
			pending_currency: b"chf".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};

		assert_ok!(StripeBridge::submit_balance_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert!(!BalanceCheckRequested::<Test>::get());
		let info = LastBalance::<Test>::get().expect("should exist");
		assert_eq!(info.available_amount, 50000);
		assert_eq!(info.pending_amount, 10000);
	});
}

// ===========================================================================
//  Section 2: Offchain worker tests (mocked HTTP + Stripe API)
// ===========================================================================

// Verifies that the Stripe API key can be read back from persistent offchain storage.
#[test]
fn offchain_api_key_readable() {
	let (mut ext, _offchain_state, _pool_state) = new_test_ext_with_offchain();

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);

		let key = sp_io::offchain::local_storage_get(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
		);
		assert!(key.is_some(), "API key should be readable from offchain storage");
		assert_eq!(key.unwrap(), b"sk_test_demo_key");
	});
}

// Verifies that the offchain worker scans canonical outbound bridge transfers and submits a result.
#[test]
fn offchain_worker_processes_outbound_bridge_transfer() {
	let (mut ext, offchain_state, pool_state) = new_test_ext_with_offchain();

	{
		let mut state = offchain_state.write();
		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "POST".into(),
			uri: "https://api.stripe.com/v1/payment_intents".into(),
			headers: vec![
				("Content-Type".into(), "application/x-www-form-urlencoded".into()),
				("Authorization".into(), "Bearer sk_test_demo_key".into()),
				("Idempotency-Key".into(), "stripe-bridge-outbound-0".into()),
			],
			body: b"amount=1000&currency=chf&confirm=true&payment_method=pm_card_visa&payment_method_types[]=card".to_vec(),
			response: Some(
				br#"{"id":"pi_outbound_ocw_001","status":"succeeded","amount":1000,"currency":"chf"}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});
	}

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		StripeEnabled::<Test>::put(true);

		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			1000,
			b"chf".to_vec(),
		));

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	let tx = pool_state.write().transactions.pop();
	assert!(tx.is_some(), "expected an outbound bridge transfer result transaction");
}

// Verifies that a fresh local in-flight marker suppresses immediate outbound reprocessing.
#[test]
fn offchain_worker_skips_outbound_bridge_transfer_with_fresh_inflight_marker() {
	let (mut ext, _offchain_state, pool_state) = new_test_ext_with_offchain();

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		set_remuneration_balance(bob(), 1000);
		StripeEnabled::<Test>::put(true);

		assert_ok!(StripeBridge::request_transfer_to_stripe(
			RuntimeOrigin::signed(alice()),
			bob(),
			1000,
			b"chf".to_vec(),
		));

		let now_ms = sp_io::offchain::timestamp().unix_millis();
		let storage_key =
			[crate::OUTBOUND_TRANSFER_IN_FLIGHT_PREFIX, b"::", &0u64.encode()].concat();
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			&storage_key,
			&now_ms.encode(),
		);

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	assert!(
		pool_state.write().transactions.is_empty(),
		"no tx should be submitted while an outbound transfer is locally in-flight"
	);
}

// Verifies that stale local in-flight markers expire conservatively.
#[test]
fn outbound_inflight_marker_staleness_logic_is_correct() {
	new_test_ext().execute_with(|| {
		assert!(StripeBridge::is_outbound_transfer_in_flight_marker_fresh(
			1_000,
			1_000 + OUTBOUND_TRANSFER_IN_FLIGHT_TTL_MS - 1,
		));
		assert!(!StripeBridge::is_outbound_transfer_in_flight_marker_fresh(
			1_000,
			1_000 + OUTBOUND_TRANSFER_IN_FLIGHT_TTL_MS,
		));
	});
}

// Verifies that the offchain worker processes one queued payment and submits a result transaction.
#[test]
fn offchain_worker_processes_pending_payment() {
	let (mut ext, offchain_state, pool_state) = new_test_ext_with_offchain();

	{
		let mut state = offchain_state.write();
		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "POST".into(),
			uri: "https://api.stripe.com/v1/payment_intents".into(),
			headers: vec![
				("Content-Type".into(), "application/x-www-form-urlencoded".into()),
				("Authorization".into(), "Bearer sk_test_demo_key".into()),
			],
			body: b"amount=1000&currency=chf&confirm=true&payment_method=pm_card_visa&payment_method_types[]=card".to_vec(),
			response: Some(
				br#"{"id":"pi_mock_001","status":"succeeded","amount":1000,"currency":"chf"}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});

		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "GET".into(),
			uri: "https://api.stripe.com/v1/payment_intents/pi_mock_001?expand[]=latest_charge.balance_transaction".into(),
			headers: vec![
				("Authorization".into(), "Bearer sk_test_demo_key".into()),
			],
			body: vec![],
			response: Some(
				br#"{"id":"pi_mock_001","status":"succeeded","amount":1000,"latest_charge":{"balance_transaction":{"amount":1000,"fee":29,"net":971,"currency":"chf"}}}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});
	}

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		StripeEnabled::<Test>::put(true);

		assert_ok!(StripeBridge::queue_stripe_payment(
			RuntimeOrigin::signed(alice()),
			bob(),
			1000,
			b"chf".to_vec(),
		));

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	let tx = pool_state.write().transactions.pop();
	assert!(tx.is_some(), "expected a transaction in the pool");
}

// Verifies that the offchain worker exits without submitting transactions when the bridge is disabled.
#[test]
fn offchain_worker_skips_when_disabled() {
	let (mut ext, _offchain_state, pool_state) = new_test_ext_with_offchain();

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	assert!(pool_state.write().transactions.is_empty(), "no tx should be submitted when disabled");
}

// Verifies that the offchain worker exits without submitting transactions when no API key is configured.
#[test]
fn offchain_worker_skips_without_api_key() {
	let (mut ext, _offchain_state, pool_state) = new_test_ext_with_offchain();

	ext.execute_with(|| {
		set_custodian(alice());
		StripeEnabled::<Test>::put(true);

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	assert!(
		pool_state.write().transactions.is_empty(),
		"no tx should be submitted without API key"
	);
}

// Verifies that the offchain worker fetches a Stripe balance snapshot when a balance check is requested.
#[test]
fn offchain_worker_processes_balance_check() {
	let (mut ext, offchain_state, pool_state) = new_test_ext_with_offchain();

	{
		let mut state = offchain_state.write();
		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "GET".into(),
			uri: "https://api.stripe.com/v1/balance".into(),
			headers: vec![
				("Authorization".into(), "Bearer sk_test_demo_key".into()),
			],
			body: vec![],
			response: Some(
				br#"{"available":[{"amount":50000,"currency":"chf"}],"pending":[{"amount":10000,"currency":"chf"}]}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});
	}

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		StripeEnabled::<Test>::put(true);
		BalanceCheckRequested::<Test>::put(true);

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	let tx = pool_state.write().transactions.pop();
	assert!(tx.is_some(), "expected a balance result transaction");
}

// Verifies that the offchain worker processes one queued refund and submits a refund result transaction.
#[test]
fn offchain_worker_processes_refund() {
	let (mut ext, offchain_state, pool_state) = new_test_ext_with_offchain();

	{
		let mut state = offchain_state.write();
		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "GET".into(),
			uri: "https://api.stripe.com/v1/payment_intents/pi_test_refund".into(),
			headers: vec![("Authorization".into(), "Bearer sk_test_demo_key".into())],
			body: vec![],
			response: Some(
				br#"{"id":"pi_test_refund","status":"succeeded","latest_charge":"ch_test_charge"}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});

		state.expect_request(sp_core::offchain::testing::PendingRequest {
			method: "POST".into(),
			uri: "https://api.stripe.com/v1/refunds".into(),
			headers: vec![
				("Content-Type".into(), "application/x-www-form-urlencoded".into()),
				("Authorization".into(), "Bearer sk_test_demo_key".into()),
			],
			body: b"charge=ch_test_charge&reason=requested_by_customer".to_vec(),
			response: Some(
				br#"{"id":"re_test_001","status":"succeeded","amount":1000,"currency":"chf"}"#
					.to_vec(),
			),
			response_headers: vec![("content-type".into(), "application/json".into())],
			sent: true,
			..Default::default()
		});
	}

	ext.execute_with(|| {
		sp_io::offchain::local_storage_set(
			sp_core::offchain::StorageKind::PERSISTENT,
			STRIPE_API_KEY_STORAGE,
			b"sk_test_demo_key",
		);
		set_custodian(alice());
		StripeEnabled::<Test>::put(true);

		let record = StripePaymentRecord {
			stripe_payment_id: b"pi_test_refund".to_vec().try_into().unwrap(),
			status: b"succeeded".to_vec().try_into().unwrap(),
			gross_amount: 1000,
			stripe_fee: 29,
			net_amount: 971,
		};
		ProcessedPayments::<Test>::insert(0u64, record);

		assert_ok!(StripeBridge::queue_stripe_refund(RuntimeOrigin::signed(alice()), 0,));

		<StripeBridge as Hooks<u64>>::offchain_worker(1u64);
	});

	let tx = pool_state.write().transactions.pop();
	assert!(tx.is_some(), "expected a refund result transaction");
}

// ===========================================================================
//  Section 3: Integration tests (remuneration <-> stripe-bridge)
// ===========================================================================

// Verifies that the remuneration custodian controls administrative access to the Stripe bridge.
#[test]
fn remuneration_custodian_controls_stripe_bridge() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());

		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_noop!(
			StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(bob()), false),
			crate::Error::<Test>::NotCustodian
		);
	});
}

// Verifies that remuneration state remains accessible in the integrated Stripe bridge mock runtime.
#[test]
fn remuneration_balance_visible_from_stripe_bridge() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());

		assert_ok!(Remuneration::add_community(
			RuntimeOrigin::signed(alice()),
			charlie(),
			alice(),
			alice(),
		));
		assert_ok!(Remuneration::add_prosumer(RuntimeOrigin::signed(alice()), bob(), charlie(),));

		assert_ok!(Remuneration::set_balance(RuntimeOrigin::signed(alice()), bob(), 5000u128,));

		let balance = remuneration::Pallet::<Test>::query_balance(bob());
		assert_eq!(balance, 5000u128);
	});
}

// Verifies that processed Stripe payment records can be retrieved through the pallet query helper.
#[test]
fn stripe_payment_result_is_queryable() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		assert_ok!(StripeBridge::queue_stripe_payment(
			RuntimeOrigin::signed(alice()),
			bob(),
			500,
			b"usd".to_vec(),
		));

		let record = StripePaymentRecord {
			stripe_payment_id: b"pi_query_test".to_vec().try_into().unwrap(),
			status: b"succeeded".to_vec().try_into().unwrap(),
			gross_amount: 500,
			stripe_fee: 15,
			net_amount: 485,
		};
		ProcessedPayments::<Test>::insert(0u64, record);
		PendingPayments::<Test>::remove(0u64);

		let result = StripeBridge::query_processed_payment(0);
		assert!(result.is_some());
		let r = result.unwrap();
		assert_eq!(r.stripe_payment_id.as_slice(), b"pi_query_test");
		assert_eq!(r.gross_amount, 500);
		assert_eq!(r.net_amount, 485);
	});
}

// Verifies the full payment lifecycle from queueing a payment through storing a processed refund.
#[test]
fn full_payment_lifecycle() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		// 1. Queue
		assert_ok!(StripeBridge::queue_stripe_payment(
			RuntimeOrigin::signed(alice()),
			bob(),
			2000,
			b"chf".to_vec(),
		));
		assert!(PendingPayments::<Test>::get(0).is_some());
		assert!(ProcessedPayments::<Test>::get(0).is_none());

		// 2. Simulate OCW submitting result
		let payload = crate::pallet::PaymentResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			payment_index: 0,
			stripe_payment_id: b"pi_lifecycle_test".to_vec(),
			status: b"succeeded".to_vec(),
			gross_amount: 2000,
			stripe_fee: 58,
			net_amount: 1942,
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_payment_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		// 3. Verify lifecycle transition
		assert!(PendingPayments::<Test>::get(0).is_none());
		let record = ProcessedPayments::<Test>::get(0).expect("should exist");
		assert_eq!(record.status.as_slice(), b"succeeded");

		// 4. Queue a refund
		assert_ok!(StripeBridge::queue_stripe_refund(RuntimeOrigin::signed(alice()), 0,));
		let refund_req = PendingRefunds::<Test>::get(0).expect("should exist");
		assert_eq!(refund_req.stripe_payment_id.as_slice(), b"pi_lifecycle_test");

		// 5. Simulate OCW submitting refund result
		let refund_payload = crate::pallet::RefundResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			refund_index: 0,
			refund_id: b"re_lifecycle_test".to_vec(),
			status: b"succeeded".to_vec(),
			amount: 2000,
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_refund_result(
			RuntimeOrigin::none(),
			refund_payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		assert!(PendingRefunds::<Test>::get(0).is_none());
		let refund_record = ProcessedRefunds::<Test>::get(0).expect("should exist");
		assert_eq!(refund_record.refund_id.as_slice(), b"re_lifecycle_test");
		assert_eq!(refund_record.amount, 2000);
	});
}

// Verifies the full balance-check lifecycle from requesting a snapshot to storing the returned balance.
#[test]
fn full_balance_check_lifecycle() {
	new_test_ext().execute_with(|| {
		set_custodian(alice());
		assert_ok!(StripeBridge::set_stripe_enabled(RuntimeOrigin::signed(alice()), true));

		// 1. Request balance check
		assert_ok!(StripeBridge::request_balance_check(RuntimeOrigin::signed(alice())));
		assert!(BalanceCheckRequested::<Test>::get());

		// 2. Simulate OCW balance result
		let payload = crate::pallet::BalanceResultPayload::<
			<Test as frame_system::offchain::SigningTypes>::Public,
		> {
			available_amount: 100000,
			available_currency: b"chf".to_vec(),
			pending_amount: 25000,
			pending_currency: b"chf".to_vec(),
			public: sp_core::sr25519::Public::from_raw([99u8; 32]).into(),
		};
		assert_ok!(StripeBridge::submit_balance_result(
			RuntimeOrigin::none(),
			payload,
			sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
		));

		// 3. Verify
		assert!(!BalanceCheckRequested::<Test>::get());
		let info = LastBalance::<Test>::get().expect("should exist");
		assert_eq!(info.available_amount, 100000);
		assert_eq!(info.available_currency.as_slice(), b"chf");
		assert_eq!(info.pending_amount, 25000);
	});
}
