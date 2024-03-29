use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use sp_core::H256;
use std::str::FromStr;

#[test]
fn add_already_registered_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register a proxy.
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(OrderbookRegistry::is_registered_proxy_account(&ALICE, BOB), true);
		assert_noop!(
			OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
	});
}

#[test]
fn add_matching_engine_operator_works() {
	new_test_ext().execute_with(|| {
		// Register a matching_engine operator.
		assert_ok!(OrderbookRegistry::register_matching_engine_operator(Origin::root(), ALICE));
		assert_noop!(
			OrderbookRegistry::register_matching_engine_operator(Origin::root(), ALICE),
			Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn add_more_then_three_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Add proxies.
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), CHARLIE));
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), MIKE));
		assert_noop!(
			OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), JOHN),
			Error::<Test>::ProxyAccountsLimitReached
		);
	});
}

#[test]
fn add_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register a proxy.
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(OrderbookRegistry::is_registered_proxy_account(&ALICE, BOB), true);
	});
}

#[test]
fn add_self_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Add proxies.
		assert_noop!(
			OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), ALICE),
			Error::<Test>::NoSelfProxy
		);
	});
}

#[test]
fn add_user_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		assert_noop!(
			OrderbookRegistry::register_user(Origin::root(), ALICE),
			Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn delegator_must_be_a_registered_user() {
	new_test_ext().execute_with(|| {
		// Add a proxy from an unregistered user.
		assert_noop!(
			OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), CHARLIE),
			Error::<Test>::NotARegisteredUserAccount
		);
	});
}

#[test]
fn delete_orders_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			Origin::signed(ALICE),
			orders_hash.clone()
		));
		assert_ok!(OrderbookRegistry::delete_orders(
			Origin::signed(ALICE),
			orders_hash
		));
	});
}

#[test]
fn delete_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register proxy
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			Origin::signed(BOB),
			ALICE,
			orders_hash.clone()
		));
		assert_ok!(OrderbookRegistry::delete_orders_by_proxy(
			Origin::signed(BOB),
			ALICE,
			orders_hash
		));
	});
}

#[test]
fn insert_orders_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			Origin::signed(ALICE),
			orders_hash
		));
	});
}

#[test]
fn insert_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register proxy
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			Origin::signed(BOB),
			ALICE,
			orders_hash
		));
	});
}

#[test]
fn insert_same_orders_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			Origin::signed(ALICE),
			orders_hash.clone()
		));
		assert_noop!(OrderbookRegistry::insert_orders(
			Origin::signed(ALICE),
			orders_hash),
			Error::<Test>::OrderAlreadyInserted
		);
	});
}

#[test]
fn insert_same_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register proxy
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			Origin::signed(BOB),
			ALICE,
			orders_hash.clone()
		));
		assert_noop!(OrderbookRegistry::insert_orders_by_proxy(
			Origin::signed(BOB),
			ALICE,
			orders_hash),
			Error::<Test>::OrderAlreadyInserted
		);
	});
}

#[test]
fn registered_matching_engine_operator_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a matching_engine operator.
		assert_noop!(
			OrderbookRegistry::register_matching_engine_operator(Origin::signed(ALICE), BOB), 
			BadOrigin
		);
	});
}

#[test]
fn registered_user_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_noop!(OrderbookRegistry::register_user(Origin::signed(ALICE), BOB), BadOrigin);
	});
}

#[test]
fn remove_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(OrderbookRegistry::register_user(Origin::root(), ALICE));
		// Register a proxy.
		assert_ok!(OrderbookRegistry::register_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(OrderbookRegistry::is_registered_proxy_account(&ALICE, BOB), true);
		// Remove proxies.
		assert_ok!(OrderbookRegistry::unregister_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(OrderbookRegistry::is_registered_proxy_account(&ALICE, BOB), false);
	});
}