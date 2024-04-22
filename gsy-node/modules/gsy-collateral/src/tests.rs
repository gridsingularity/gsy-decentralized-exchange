use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn add_user_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(Origin::root(), ALICE));
		assert_noop!(
			GsyCollateral::register_user(Origin::root(), ALICE),
			Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn add_matching_engine_operator_works() {
	new_test_ext().execute_with(|| {
		// Register a matching_engine operator.
		assert_ok!(GsyCollateral::register_matching_engine_operator(Origin::root(), ALICE));
		assert_noop!(
			GsyCollateral::register_matching_engine_operator(Origin::root(), ALICE),
			Error::<Test>::AlreadyRegistered
		);
	});
}

#[test]
fn registered_user_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_noop!(GsyCollateral::register_user(Origin::signed(ALICE), BOB), BadOrigin);
	});
}

#[test]
fn registered_matching_engine_operator_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a matching_engine operator.
		assert_noop!(GsyCollateral::register_matching_engine_operator(Origin::signed(ALICE), BOB), BadOrigin);
	});
}
#[test]
fn add_remove_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(Origin::root(), ALICE));
		// Add proxies.
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), ALICE),
			Error::<Test>::NoSelfProxy
		);
		assert_ok!(GsyCollateral::register_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), true);
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), BOB),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::register_proxy_account(Origin::signed(ALICE), CHARLIE));
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), CHARLIE),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::register_proxy_account(Origin::signed(ALICE), MIKE));
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), MIKE),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), JOHN),
			Error::<Test>::ProxyAccountsLimitReached
		);
		// Remove proxies.
		assert_ok!(GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), false);
		assert_noop!(
			GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), BOB),
			Error::<Test>::NotARegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), CHARLIE));
		assert_noop!(
			GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), CHARLIE),
			Error::<Test>::NotARegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), MIKE));
		assert_noop!(
			GsyCollateral::unregister_proxy_account(Origin::signed(ALICE), MIKE),
			Error::<Test>::NotRegisteredProxyAccounts
		);
	});
}

#[test]
fn delegator_must_be_a_registered_user() {
	new_test_ext().execute_with(|| {
		// Add a proxy from an unregistered user.
		assert_noop!(
			GsyCollateral::register_proxy_account(Origin::signed(ALICE), CHARLIE),
			Error::<Test>::NotARegisteredUserAccount
		);
	});
}

#[test]
fn deposit_collateral_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(Origin::root(), ALICE));
		// Deposit collateral.
		assert_noop!(
			GsyCollateral::deposit_collateral(Origin::signed(BOB), 100),
			Error::<Test>::NotARegisteredUserAccount
		);
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(ALICE), 100));
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(ALICE), 900));
		assert_ok!(GsyCollateral::shutdown_vault(Origin::root(), ALICE));
		assert_noop!(
			GsyCollateral::deposit_collateral(Origin::signed(ALICE), 300),
			Error::<Test>::DepositsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(Origin::root(), ALICE));
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(ALICE), 600));
		// Register a new user with low balance.
		assert_ok!(GsyCollateral::register_user(Origin::root(), NELSON));
		assert_noop!(
			GsyCollateral::deposit_collateral(Origin::signed(NELSON), 100),
			Error::<Test>::NotEnoughBalance
		);
	});
}

#[test]
fn withdraw_collateral_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(Origin::root(), ALICE));
		// Deposit collateral.
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(ALICE), 200));
		// Withdraw collateral.
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(BOB), 100),
			Error::<Test>::NotARegisteredUserAccount
		);
		assert_ok!(GsyCollateral::withdraw_collateral(Origin::signed(ALICE), 100));
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(ALICE), 100),
			Error::<Test>::NotEnoughCollateralForFee
		);
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(ALICE), 200),
			Error::<Test>::NotEnoughCollateral
		);
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(ALICE), 100));
		assert_ok!(GsyCollateral::shutdown_vault(Origin::root(), ALICE));
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(ALICE), 100),
			Error::<Test>::WithdrawalsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(Origin::root(), ALICE));
		assert_ok!(GsyCollateral::withdraw_collateral(Origin::signed(ALICE), 100));

		// Register a new user.
		assert_ok!(GsyCollateral::register_user(Origin::root(), CHARLIE));
		// Deposit collateral.
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(CHARLIE), 200));
		// Withdraw collateral.
		assert_ok!(GsyCollateral::withdraw_collateral(Origin::signed(CHARLIE), 199));
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(CHARLIE), 30),
			Error::<Test>::NotEnoughCollateral
		);
		assert_ok!(GsyCollateral::deposit_collateral(Origin::signed(CHARLIE), 100));
		assert_ok!(GsyCollateral::shutdown_vault(Origin::root(), CHARLIE));
		assert_noop!(
			GsyCollateral::withdraw_collateral(Origin::signed(CHARLIE), 100),
			Error::<Test>::WithdrawalsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(Origin::root(), CHARLIE));
		assert_ok!(GsyCollateral::withdraw_collateral(Origin::signed(CHARLIE), 100));
	});
}
