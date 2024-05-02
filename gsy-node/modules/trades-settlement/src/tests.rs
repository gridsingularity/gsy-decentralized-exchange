use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use test_helpers::{dummy_bid, dummy_offer, dummy_trade};

#[test]
fn settle_trades_works() {
	// new_test_ext().execute_with(|| {
	// 	let bid = dummy_bid(ALICE, 100, 10);
	// 	let bid_2 = dummy_bid(ALICE, 200, 10);
	// 	let offer = dummy_offer(BOB, 100, 10);
	// 	let offer_2 = dummy_offer(BOB, 100, 50);
	// 	let offer_3 = dummy_offer(BOB, 200, 50);
	// 	// TODO: construct hashes of bid and offer from Trade struct parameters
	// 	let trade = dummy_trade(ALICE, BOB, 100, 10);
	// 	let trade_2 = dummy_trade(ALICE, BOB, 100, 10);
	// 	let unregistered_bid = dummy_bid(CHARLIE, 100, 10);
	// 	let unregistered_offer = dummy_offer(BOB, 100, 10);
	// 	let trade_with_unregistered_bid =
	// 		dummy_trade(CHARLIE, BOB, 100, 10);
	// 	let trade_with_unregistered_offer =
	// 		dummy_trade(ALICE, BOB, 100, 10);
	// 	let	trade_with_high_selected_energy =
	// 		dummy_trade(ALICE, BOB, 150, 10);
	// 	let	trade_with_high_selected_energy_2 =
	// 		dummy_trade(ALICE, BOB, 150, 10);
	// 	// Register users.
	// 	assert_ok!(GsyCollateral::register_user(Origin::root(), ALICE));
	// 	assert_ok!(GsyCollateral::register_user(Origin::root(), BOB));
	// 	// Register matching_engine operator.
	// 	assert_ok!(GsyCollateral::register_matching_engine_operator(Origin::root(), MIKE));
	// 	// Add Orders.
	// TODO: fix tests to accept Vec<BidOfferMatch>
	//	assert_ok!(OrderbookRegistry::insert_order(Origin::signed(ALICE), bid.clone().hash()));
	//	assert_ok!(OrderbookRegistry::insert_order(Origin::signed(ALICE), bid_2.clone().hash()));
	//	assert_ok!(OrderbookRegistry::insert_order(Origin::signed(BOB), offer.clone().hash()));
	//	assert_ok!(OrderbookRegistry::insert_order(Origin::signed(BOB), offer_2.clone().hash()));
	//	assert_ok!(OrderbookRegistry::insert_order(Origin::signed(BOB), offer_3.clone().hash()));


	// 	// Clear trade.
	// 	assert_ok!(TradesSettlement::settle_trades(
	// 		Origin::signed(MIKE),
	// 		bid.clone(),
	// 		offer.clone(),
	// 		trade.clone()
	// 	));
	// 	// Clear trade that has already been settled.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			bid.clone(),
	// 			offer.clone(),
	// 			trade.clone()
	// 		),
	// 		Error::<Test>::OrderNotExecutable
	// 	);
	// 	// Clear trade with unregistered bid.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			unregistered_bid.clone(),
	// 			offer.clone(),
	// 			trade_with_unregistered_bid.clone()
	// 		),
	// 		Error::<Test>::OrderNotRegistered
	// 	);
	// 	// Clear trade with unregistered offer.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			bid.clone(),
	// 			unregistered_offer.clone(),
	// 			trade_with_unregistered_offer.clone()
	// 		),
	// 		Error::<Test>::OrderNotRegistered
	// 	);
	// 	// Clear trade with offered energy lower than trade selected energy.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			bid_2.clone(),
	// 			offer.clone(),
	// 			trade_with_high_selected_energy.clone()
	// 		),
	// 		Error::<Test>::OfferEnergyLessThanSelectedEnergy
	// 	);
	// 	// Clear trade with bid energy lower than trade selected energy.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			bid.clone(),
	// 			offer_3.clone(),
	// 			trade_with_high_selected_energy_2.clone()
	// 		),
	// 		Error::<Test>::BidEnergyLessThanSelectedEnergy
	// 	);
	// 	// Clear trade with offered energy_rate higher than bid energy_rate.
	// 	assert_noop!(
	// 		TradesSettlement::settle_trades(
	// 			Origin::signed(MIKE),
	// 			bid.clone(),
	// 			offer_2.clone(),
	// 			trade_2.clone()
	// 		),
	// 		Error::<Test>::OfferEnergyRateGreaterThanBidEnergyRate
	// 	);
	// });
}
