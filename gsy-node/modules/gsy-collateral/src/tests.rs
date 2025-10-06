use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;
use frame_system::RawOrigin;

#[test]
fn add_user_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		assert_noop!(
			GsyCollateral::register_user(RawOrigin::Root.into(), ALICE),
			Error::<Test>::AlreadyRegistered
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
			Error::<Test>::AlreadyRegistered
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
fn registered_exchange_operator_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register an exchange operator.
		assert_noop!(GsyCollateral::register_exchange_operator(
			RawOrigin::Signed(ALICE).into(), BOB), BadOrigin);
	});
}
#[test]
fn add_remove_proxies_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Add proxies.
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), ALICE),
			Error::<Test>::NoSelfProxy
		);
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), true);
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), BOB),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE));
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), MIKE));
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), MIKE),
			Error::<Test>::AlreadyRegisteredProxyAccount
		);
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), JOHN),
			Error::<Test>::ProxyAccountsLimitReached
		);
		// Remove proxies.
		assert_ok!(GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), BOB));
		assert_eq!(GsyCollateral::is_registered_proxy_account(&ALICE, BOB), false);
		assert_noop!(
			GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), BOB),
			Error::<Test>::NotARegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE));
		assert_noop!(
			GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE),
			Error::<Test>::NotARegisteredProxyAccount
		);
		assert_ok!(GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), MIKE));
		assert_noop!(
			GsyCollateral::unregister_proxy_account(RawOrigin::Signed(ALICE).into(), MIKE),
			Error::<Test>::NotRegisteredProxyAccounts
		);
	});
}

#[test]
fn delegator_must_be_a_registered_user() {
	new_test_ext().execute_with(|| {
		// Add a proxy from an unregistered user.
		assert_noop!(
			GsyCollateral::register_proxy_account(RawOrigin::Signed(ALICE).into(), CHARLIE),
			Error::<Test>::NotARegisteredUserAccount
		);
	});
}

#[test]
fn deposit_collateral_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Deposit collateral.
		assert_noop!(
			GsyCollateral::deposit_collateral(RawOrigin::Signed(BOB).into(), 100),
			Error::<Test>::NotARegisteredUserAccount
		);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 100));
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 900));
		assert_ok!(GsyCollateral::shutdown_vault(RawOrigin::Root.into(), ALICE));
		assert_noop!(
			GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 300),
			Error::<Test>::DepositsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(RawOrigin::Root.into(), ALICE));
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 600));
		// Register a new user with low balance.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), NELSON));
		assert_noop!(
			GsyCollateral::deposit_collateral(RawOrigin::Signed(NELSON).into(), 100),
			Error::<Test>::NotEnoughBalance
		);
	});
}

#[test]
fn withdraw_collateral_works() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), ALICE));
		// Deposit collateral.
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 200));
		// Withdraw collateral.
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(BOB).into(), 100),
			Error::<Test>::NotARegisteredUserAccount
		);
		assert_ok!(GsyCollateral::withdraw_collateral(RawOrigin::Signed(ALICE).into(), 100));
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(ALICE).into(), 100),
			Error::<Test>::NotEnoughCollateralForFee
		);
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(ALICE).into(), 200),
			Error::<Test>::NotEnoughCollateral
		);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 100));
		assert_ok!(GsyCollateral::shutdown_vault(RawOrigin::Root.into(), ALICE));
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(ALICE).into(), 100),
			Error::<Test>::WithdrawalsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(RawOrigin::Root.into(), ALICE));
		assert_ok!(GsyCollateral::withdraw_collateral(RawOrigin::Signed(ALICE).into(), 100));

		// Register a new user.
		assert_ok!(GsyCollateral::register_user(RawOrigin::Root.into(), CHARLIE));
		// Deposit collateral.
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(CHARLIE).into(), 200));
		// Withdraw collateral.
		assert_ok!(GsyCollateral::withdraw_collateral(RawOrigin::Signed(CHARLIE).into(), 199));
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(CHARLIE).into(), 30),
			Error::<Test>::NotEnoughCollateral
		);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(CHARLIE).into(), 100));
		assert_ok!(GsyCollateral::shutdown_vault(RawOrigin::Root.into(), CHARLIE));
		assert_noop!(
			GsyCollateral::withdraw_collateral(RawOrigin::Signed(CHARLIE).into(), 100),
			Error::<Test>::WithdrawalsNotAllowed
		);
		assert_ok!(GsyCollateral::restart_vault(RawOrigin::Root.into(), CHARLIE));
		assert_ok!(GsyCollateral::withdraw_collateral(RawOrigin::Signed(CHARLIE).into(), 100));
	});
}
