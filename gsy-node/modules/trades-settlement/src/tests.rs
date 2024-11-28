use crate::{mock::*, Error};
use frame_system::{RawOrigin, Origin};
use frame_support::{assert_noop, assert_ok, traits::fungible::Mutate};
use sp_runtime::traits::BlakeTwo256;
use gsy_primitives::{BidOfferMatch, HashT};
use crate::test_orders::TestOrderbookFunctions;
use crate::mock::OrderbookRegistry;
use crate::mock::GsyCollateral;

#[test]
fn settle_trades_works() {
	new_test_ext().execute_with(|| {
		let bid = TestOrderbookFunctions::dummy_bid::<Test>(ALICE, 2, 100, 10);
		let bid_2 = TestOrderbookFunctions::dummy_bid::<Test>(ALICE, 2, 200, 10);
		let offer = TestOrderbookFunctions::dummy_offer::<Test>(BOB, 2,  100, 10);
		let offer_2 = TestOrderbookFunctions::dummy_offer::<Test>(BOB,2,  100, 50);
		let offer_3 = TestOrderbookFunctions::dummy_offer::<Test>(BOB,2,  200, 50);
		// TODO: construct hashes of bid and offer from Trade struct parameters
		// let trade = TestOrderbookFunctions::dummy_trade::<Test>(ALICE, BOB, 100, 10);
		// let trade_2 = TestOrderbookFunctions::dummy_trade::<Test>(ALICE, BOB, 100, 10);
		let unregistered_bid = TestOrderbookFunctions::dummy_bid::<Test>(CHARLIE,6, 100, 10);
		let unregistered_offer = TestOrderbookFunctions::dummy_offer::<Test>(BOB,7, 100, 10);
		// let trade_with_unregistered_bid =
		// 	TestOrderbookFunctions::dummy_trade::<Test>(CHARLIE, BOB, 100, 10);
		// let trade_with_unregistered_offer =
		// 	TestOrderbookFunctions::dummy_trade::<Test>(ALICE, BOB, 100, 10);
		// let	trade_with_high_selected_energy =
		// 	TestOrderbookFunctions::dummy_trade::<Test>(ALICE, BOB, 150, 10);
		// let	trade_with_high_selected_energy_2 =
		// 	TestOrderbookFunctions::dummy_trade::<Test>(ALICE, BOB, 150, 10);
		// Register users.
		assert_ok!(OrderbookRegistry::register_user(RawOrigin::Root.into(), ALICE));
		// Add Orders.
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(ALICE).into(), vec!(BlakeTwo256::hash_of(&bid.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(ALICE).into(), vec!(BlakeTwo256::hash_of(&bid_2.clone()))));

		assert_ok!(OrderbookRegistry::register_user(RawOrigin::Root.into(), BOB));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer_2.clone()))));
		assert_ok!(OrderbookRegistry::insert_orders(RawOrigin::Signed(BOB).into(), vec!(BlakeTwo256::hash_of(&offer_3.clone()))));

		// Register matching_engine operator.
		assert_ok!(OrderbookRegistry::register_matching_engine_operator(RawOrigin::Root.into(), MIKE));
		let bid_offer_match = BidOfferMatch{
			market_id: 1,
			time_slot: bid.bid_component.time_slot,
			bid: bid.clone(),
			offer: offer.clone(),
			residual_offer: None,
			residual_bid: None,
			selected_energy: 100,
			energy_rate: 10,
		};

		let trade_vector = vec!(bid_offer_match.clone());

		// assert_ok!(GsyCollateral::create_vault(ALICE));
		// Balances::set_balance(&ALICE, 10000);
		// assert_ok!(GsyCollateral::deposit_collateral(RawOrigin::Signed(ALICE).into(), 100));
		// Clear trade.
		// assert_ok!(TradesSettlement::settle_trades(
		// 	RawOrigin::Signed(MIKE).into(), trade_vector));
		// // Clear trade that has already been settled.
		// // Recreate vector since the former one was moved
		// let trade_vector = vec!(bid_offer_match.clone());
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(), trade_vector),
		// 	Error::<Test>::OrdersNotExecutable
		// );
		//
		// let bid_offer_match_unregistered_bid = BidOfferMatch{
		// 	market_id: 1,
		// 	time_slot: unregistered_bid.bid_component.time_slot,
		// 	bid: unregistered_bid,
		// 	offer: offer.clone(),
		// 	residual_offer: None,
		// 	residual_bid: None,
		// 	selected_energy: 13,
		// 	energy_rate: 12,
		// };
		// let trade_vector = vec!(bid_offer_match_unregistered_bid);
		//
		// // Clear trade with unregistered bid.
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(), trade_vector),
		// 	Error::<Test>::OrdersNotRegistered
		// );
		//
		// let bid_offer_match_unregistered_offer = BidOfferMatch{
		// 	market_id: 1,
		// 	time_slot: offer.offer_component.time_slot,
		// 	bid: bid.clone(),
		// 	offer: unregistered_offer.clone(),
		// 	residual_offer: None,
		// 	residual_bid: None,
		// 	selected_energy: 100,
		// 	energy_rate: 10,
		// };
		// let trade_vector = vec!(bid_offer_match_unregistered_offer);
		// // Clear trade with unregistered offer.
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(), trade_vector),
		// 	Error::<Test>::OrdersNotRegistered
		// );

		// // Clear trade with offered energy lower than trade selected energy.
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(),
		// 		bid_2.clone(),
		// 		offer.clone(),
		// 		trade_with_high_selected_energy.clone()
		// 	),
		// 	Error::<Test>::OfferEnergyLessThanSelectedEnergy
		// );
		// // Clear trade with bid energy lower than trade selected energy.
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(),
		// 		bid.clone(),
		// 		offer_3.clone(),
		// 		trade_with_high_selected_energy_2.clone()
		// 	),
		// 	Error::<Test>::BidEnergyLessThanSelectedEnergy
		// );
		// // Clear trade with offered energy_rate higher than bid energy_rate.
		// assert_noop!(
		// 	TradesSettlement::settle_trades(
		// 		RawOrigin::Signed(MIKE).into(),
		// 		bid.clone(),
		// 		offer_2.clone(),
		// 		trade_2.clone()
		// 	),
		// 	Error::<Test>::OfferEnergyRateGreaterThanBidEnergyRate
		// );
	});
}
