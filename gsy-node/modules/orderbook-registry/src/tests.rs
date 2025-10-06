use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use sp_core::H256;
use std::str::FromStr;
use frame_system::RawOrigin;

#[test]
fn add_already_registered_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register a proxy.
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), true);
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB),
			gsy_collateral::Error::<Test>::AlreadyRegisteredProxyAccount
		);
	});
}

#[test]
fn add_exchange_operator_works() {
	new_test_ext().execute_with(|| {
		// Register an exchange operator.
		assert_ok!(GsyCollateral::register_exchange_operator(RawOrigin::Root.into(), ALICE));
		assert_noop!(
			GsyCollateral::register_exchange_operator(RawOrigin::Root.into(), ALICE),
			gsy_collateral::Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn add_more_then_three_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Add proxies.
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE));
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), MIKE));
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), JOHN),
			gsy_collateral::Error::<Test>::ProxyAccountsLimitReached
		);
	});
}

#[test]
fn add_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register a proxy.
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), true);
	});
}

#[test]
fn add_self_proxies_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Add proxies.
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), ALICE),
			gsy_collateral::Error::<Test>::NoSelfProxy
		);
	});
}

#[test]
fn add_user_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		assert_noop!(
			GsyCollateral::register_user(RawOrigin::Root.into(), ALICE),
			gsy_collateral::Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn delegator_must_be_a_registered_user() {
	new_test_ext().execute_with(|| {
		// Add a proxy from an unregistered user.
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE),
			gsy_collateral::Error::<Test>::NotARegisteredUserAccount
		);
	});
}

#[test]
fn delete_orders_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			RawOrigin::Signed(ALICE).into(),
			orders_hash.clone()
		));
		assert_ok!(OrderbookRegistry::delete_orders(
			RawOrigin::Signed(ALICE).into(),
			orders_hash
		));
	});
}

#[test]
fn delete_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register proxy
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			RawOrigin::Signed(BOB).into(),
			ALICE,
			orders_hash.clone()
		));
		assert_ok!(OrderbookRegistry::delete_orders_by_proxy(
			RawOrigin::Signed(BOB).into(),
			ALICE,
			orders_hash
		));
	});
}

#[test]
fn insert_orders_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			RawOrigin::Signed(ALICE).into(),
			orders_hash
		));
	});
}

#[test]
fn insert_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register proxy
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			RawOrigin::Signed(BOB).into(),
			ALICE,
			orders_hash
		));
	});
}

#[test]
fn insert_same_orders_should_fail() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders(
			RawOrigin::Signed(ALICE).into(),
			orders_hash.clone()
		));
		assert_noop!(OrderbookRegistry::insert_orders(
			RawOrigin::Signed(ALICE).into(),
			orders_hash),
			Error::<Test>::OrderAlreadyInserted
		);
	});
}

#[test]
fn insert_same_orders_by_proxy_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register proxy
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		// Insert orders
		let mut orders_hash: Vec<H256> = Vec::new();
		let order_hash: H256 = H256::from_str(
			"0x3c80a50a11b8838f1beae03697797f54e095641f5c271d4ac19e8a7aa29a66e5"
		).unwrap();
		orders_hash.push(order_hash);
		assert_ok!(OrderbookRegistry::insert_orders_by_proxy(
			RawOrigin::Signed(BOB).into(),
			ALICE,
			orders_hash.clone()
		));
		assert_noop!(OrderbookRegistry::insert_orders_by_proxy(
			RawOrigin::Signed(BOB).into(),
			ALICE,
			orders_hash),
			Error::<Test>::OrderAlreadyInserted
		);
	});
}

#[test]
fn registered_exchange_operator_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register an exchange operator.
		assert_noop!(
			GsyCollateral::register_exchange_operator(RawOrigin::Signed(ALICE).into(), BOB),
			BadOrigin
		);
	});
}

#[test]
fn registered_user_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_noop!(GsyCollateral::register_user(RawOrigin::Signed(ALICE).into(), BOB), BadOrigin);
	});
}

#[test]
fn remove_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Register a proxy.
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), true);
		// Remove proxies.
		assert_ok!(GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), false);
	});
}