use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn registered_user_must_be_added_by_root() {
	new_test_ext().execute_with(|| {
		// Register a user.
		assert_noop!(OrderbookRegistry::register_user(Origin::signed(ALICE), BOB), BadOrigin);
	});
}
