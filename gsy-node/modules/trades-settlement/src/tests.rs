use crate::{mock::*, Error};
use frame_system::RawOrigin;
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate};
use sp_runtime::traits::BlakeTwo256;
use gsy_primitives::HashT;
use crate::test_orders::TestOrderbookFunctions;
use crate::mock::OrderbookRegistry;
use crate::mock::GsyCollateral;

#[test]
fn settle_trades_works() {
	new_test_ext().execute_with(|| {

		// Register users.
		assert_ok!(TestOrderbookFunctions::add_user::<Test>(ALICE));
		assert_ok!(TestOrderbookFunctions::add_user::<Test>(BOB));
		assert_ok!(TestOrderbookFunctions::add_user::<Test>(MIKE));

		// Register matching_engine operator.
		assert_ok!(TestOrderbookFunctions::add_matching_engine_operator::<Test>(MIKE));

		// Add wallet balance and collateral
		assert_ok!(GsyCollateral::create_vault(ALICE));
		Balances::set_balance(&ALICE, 10000);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 5000));

		assert_ok!(GsyCollateral::create_vault(MIKE));
		Balances::set_balance(&MIKE, 10000);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(MIKE).into(), 5000));

		assert_ok!(GsyCollateral::create_vault(BOB));
		Balances::set_balance(&BOB, 10000);
		assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(BOB).into(), 5000));

		// Add Orders.
		let bid = TestOrderbookFunctions::dummy_bid::<Test>(ALICE, 2, 100, 10);
		let bid_2 = TestOrderbookFunctions::dummy_bid::<Test>(ALICE, 2, 100, 50);
		let bid_3 = TestOrderbookFunctions::dummy_bid::<Test>(ALICE, 2, 200, 10);
		let offer = TestOrderbookFunctions::dummy_offer::<Test>(BOB, 2,  100, 10);
		let offer_2 = TestOrderbookFunctions::dummy_offer::<Test>(BOB,2,  100, 50);
		let offer_3 = TestOrderbookFunctions::dummy_offer::<Test>(BOB,2,  200, 50);
		let unregistered_bid = TestOrderbookFunctions::dummy_bid::<Test>(CHARLIE,6, 100, 10);
		let unregistered_offer = TestOrderbookFunctions::dummy_offer::<Test>(BOB,7, 100, 10);

		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(ALICE).into(), vec!(BlakeTwo256::hash_of(&bid.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(ALICE).into(), vec!(BlakeTwo256::hash_of(&bid_2.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(ALICE).into(), vec!(BlakeTwo256::hash_of(&bid_3.clone()))));

		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer_2.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer_3.clone()))));

		// Add bid offer matches
		let bid_offer_match = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			bid.clone(), offer.clone(), None, None, 2, 100, 10);

		let bid_offer_match_unregistered_bid = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			unregistered_bid.clone(), offer.clone(), None, None, 2, 13, 12);

		let bid_offer_match_unregistered_offer = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			bid.clone(), unregistered_offer.clone(), None, None, 2, 100, 10);

		let bid_offer_match_high_selected_energy = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			bid_2.clone(), offer_2.clone(), None, None, 2, 150, 10);

		let bid_offer_match_low_selected_energy = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			bid_2.clone(), offer_2.clone(), None, None, 2, 150, 10);

		let bid_offer_match_high_energy_rate = TestOrderbookFunctions::dummy_bid_offer_match::<Test>(
			bid_3.clone(), offer_3.clone(), None, None, 2, 150, 10);

		// Clear trade.
		assert_ok!(TradesSettlement::settle_trades(
			RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match.clone())));

		// Clear trade that has already been settled.
		// Recreate vector since the former one was moved
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match.clone())),
			orderbook_registry::Error::<Test>::OrderAlreadyExecuted
		);

		// Clear trade with unregistered bid.
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match_unregistered_bid)),
			orderbook_registry::Error::<Test>::OpenOrderNotFound
		);

		// Clear trade with unregistered offer.
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match_unregistered_offer)),
			orderbook_registry::Error::<Test>::OrderAlreadyExecuted
		);

		// Clear trade with offered energy lower than trade selected energy.
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match_high_selected_energy)
			),
			Error::<Test>::NoValidMatchToSettle
		);

		// Clear trade with bid energy lower than trade selected energy.
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match_low_selected_energy)
			),
			Error::<Test>::NoValidMatchToSettle
		);

		// Clear trade with offered energy_rate higher than bid energy_rate.
		assert_noop!(
			TradesSettlement::settle_trades(
				RawOrigin::Signed(MIKE).into(), vec!(bid_offer_match_high_energy_rate)
			),
			Error::<Test>::NoValidMatchToSettle
		);
	});
}
