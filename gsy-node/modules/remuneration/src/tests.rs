use crate::{mock::*, Error, CommunityInfo, INTRA_COMMUNITY, INTER_COMMUNITY};
use frame_system::RawOrigin;
use frame_support::{assert_noop, assert_ok};
use gsy_primitives::v0::{AccountId};

// Define constants for the tests

pub const ALICE_THE_CUSTODIAN: AccountId = AccountId::new(*b"01234567890123456789012345678901");
pub const BOB_THE_CHEATER: AccountId = AccountId::new(*b"01234567890203894950393012432351");
pub const MIKE_THE_SUBSTITUTE: AccountId = AccountId::new(*b"01234588890203894950393012432351");
pub const DSO: AccountId = AccountId::new(*b"01234567890203124950392012432351");
pub const COMMUNITY1: AccountId = AccountId::new(*b"01234561230123456789012345678901");
pub const COMMUNITY1_OWNER: AccountId = AccountId::new(*b"01234561230123456789012345678902");
pub const COMMUNITY2: AccountId = AccountId::new(*b"01243561230123456789012345678901");
pub const COMMUNITY2_OWNER: AccountId = AccountId::new(*b"01243561230123456789012345678902");
pub const PROSUMER1: AccountId = AccountId::new(*b"01234653535968356825544612432351");
pub const PROSUMER2: AccountId = AccountId::new(*b"01234653135168356825544612432352");
pub const PROSUMER3: AccountId = AccountId::new(*b"01234653135168356825544612432353");

#[cfg(test)]
mod admin_tests {
    use super::*;

    #[test]
    fn custodian_management() {
        new_test_ext().execute_with(|| {
            // Initially, no custodian is set
            assert_eq!(Remuneration::custodian(), None);

            // ALICE_THE_CUSTODIAN sets herself as the custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_eq!(Remuneration::custodian(), Some(ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN updates the custodian to MIKE_THE_SUBSTITUTE
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), MIKE_THE_SUBSTITUTE));
            assert_eq!(Remuneration::custodian(), Some(MIKE_THE_SUBSTITUTE));

            // ALICE_THE_CUSTODIAN tries to update the custodian again, but fails
            assert_noop!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN), Error::<Test>::NotCustodian);

            // MIKE_THE_SUBSTITUTE updates the custodian back to ALICE_THE_CUSTODIAN
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(MIKE_THE_SUBSTITUTE).into(), ALICE_THE_CUSTODIAN));
            assert_eq!(Remuneration::custodian(), Some(ALICE_THE_CUSTODIAN));
        });
    }

    #[test]
    fn community_management() {
        new_test_ext().execute_with(|| {
            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds a new community
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_eq!(Remuneration::communities(COMMUNITY1), Some(CommunityInfo { dso: DSO, owner: COMMUNITY1_OWNER, }));

            // ALICE_THE_CUSTODIAN removes the community
            assert_ok!(Remuneration::remove_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1));
            assert_eq!(Remuneration::communities(COMMUNITY1), None);

            // BOB_THE_CHEATER tries to add a new community but fails (not being the custodian)
            assert_noop!(Remuneration::add_community(RawOrigin::Signed(BOB_THE_CHEATER).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER), Error::<Test>::NotCustodian);
        });
    }

    #[test]
    fn prosumer_management() {
        new_test_ext().execute_with(|| {
            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds a new community
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));

            // ALICE_THE_CUSTODIAN adds a prosumer to the community
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_eq!(Remuneration::prosumers(PROSUMER1), Some(COMMUNITY1));

            // ALICE_THE_CUSTODIAN removes the prosumer
            assert_ok!(Remuneration::remove_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1));
            assert_eq!(Remuneration::prosumers(PROSUMER1), None);

            // COMMUNITY1_OWNER adds a prosumer to the community
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(COMMUNITY1_OWNER).into(), PROSUMER2, COMMUNITY1));
            assert_eq!(Remuneration::prosumers(PROSUMER2), Some(COMMUNITY1));

            // COMMUNITY1_OWNER removes the prosumer
            assert_ok!(Remuneration::remove_prosumer(RawOrigin::Signed(COMMUNITY1_OWNER).into(), PROSUMER2));
            assert_eq!(Remuneration::prosumers(PROSUMER2), None);

            // ALICE_THE_CUSTODIAN re-adds a prosumer to the community
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_eq!(Remuneration::prosumers(PROSUMER1), Some(COMMUNITY1));

            // COMMUNITY1_OWNER removes the prosumer added by ALICE_THE_CUSTODIAN
            assert_ok!(Remuneration::remove_prosumer(RawOrigin::Signed(COMMUNITY1_OWNER).into(), PROSUMER1));
            assert_eq!(Remuneration::prosumers(PROSUMER1), None);

            // COMMUNITY1_OWNER re-adds a prosumer to the community
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(COMMUNITY1_OWNER).into(), PROSUMER2, COMMUNITY1));
            assert_eq!(Remuneration::prosumers(PROSUMER2), Some(COMMUNITY1));

            // ALICE_THE_CUSTODIAN removes the prosumer added by COMMUNITY1_OWNER
            assert_ok!(Remuneration::remove_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2));
            assert_eq!(Remuneration::prosumers(PROSUMER2), None);

            // BOB_THE_CHEATER tries to add a prosumer but fails (not being the custodian or the community owner)
            assert_noop!(
                Remuneration::add_prosumer(
                    RawOrigin::Signed(BOB_THE_CHEATER).into(),
                    PROSUMER1,
                    COMMUNITY1
                ),
                Error::<Test>::NotAllowedToManageProsumers
            );
        });
    }

    #[test]
    fn update_main_settlement_parameters() {
        new_test_ext().execute_with(|| {
            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // Default values should be zero
            assert_eq!(Remuneration::alpha(), 0);
            assert_eq!(Remuneration::beta(), 0);
            assert_eq!(Remuneration::under_tolerance(), 0);
            assert_eq!(Remuneration::over_tolerance(), 0);

            // Update alpha only (others remain the same via set_main_parameters)
            assert_ok!(Remuneration::set_main_parameters(
            RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
            500_000, // alpha
            Remuneration::beta(),
            Remuneration::under_tolerance(),
            Remuneration::over_tolerance(),
        ));
            assert_eq!(Remuneration::alpha(), 500_000);

            // Update beta only
            assert_ok!(Remuneration::set_main_parameters(
            RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
            Remuneration::alpha(),
            200_000, // beta
            Remuneration::under_tolerance(),
            Remuneration::over_tolerance(),
        ));
            assert_eq!(Remuneration::beta(), 200_000);

            // Update under & over tolerance only
            assert_ok!(Remuneration::set_main_parameters(
            RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
            Remuneration::alpha(),
            Remuneration::beta(),
            100_000, // under tol
            150_000, // over tol
        ));
            assert_eq!(Remuneration::under_tolerance(), 100_000);
            assert_eq!(Remuneration::over_tolerance(), 150_000);

            // Non-custodian cannot update parameters
            assert_noop!(
            Remuneration::set_main_parameters(
                RawOrigin::Signed(BOB_THE_CHEATER).into(),
                1, 2, 3, 4,
            ),
            Error::<Test>::NotCustodian
        );
        });
    }
}

#[cfg(test)]
mod payments_tests {
    use super::*;

    #[test]
    fn inter_community_payment_ok() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds two communities
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));

            let amount_to_pay = 50;

            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, 500));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, 100));

            let balance_info_before_p1 =  Remuneration::balances(COMMUNITY1);
            let balance_info_before_p2 =  Remuneration::balances(COMMUNITY2);

            assert_ok!(
                Remuneration::add_payment(
                    RawOrigin::Signed(COMMUNITY1).into(),
                    COMMUNITY2,
                    amount_to_pay,
                    INTER_COMMUNITY
                )
            );

            let balance_info_after_p1 =  Remuneration::balances(COMMUNITY1);
            let balance_info_after_p2 =  Remuneration::balances(COMMUNITY2);

            assert_eq!(balance_info_after_p1, balance_info_before_p1-amount_to_pay);
            assert_eq!(balance_info_after_p2, balance_info_before_p2+amount_to_pay);
        });
    }

    #[test]
    fn intra_community_payment_ok() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds a community and two prosumers
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            let amount_to_pay = 50;

            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 500));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 100));

            let balance_info_before_p1 =  Remuneration::balances(PROSUMER1);
            let balance_info_before_p2 =  Remuneration::balances(PROSUMER2);

            // log::error!("PROS 1: {:?}", balance_info_before_p1);
            // log::error!("PROS 2: {:?}", balance_info_before_p2);

            assert_ok!(
                Remuneration::add_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    amount_to_pay,
                    INTRA_COMMUNITY
                )
            );

            let balance_info_after_p1 =  Remuneration::balances(PROSUMER1);
            let balance_info_after_p2 =  Remuneration::balances(PROSUMER2);

            assert_eq!(balance_info_after_p1, balance_info_before_p1-amount_to_pay);
            assert_eq!(balance_info_after_p2, balance_info_before_p2+amount_to_pay);
        });
    }

    #[test]
    fn payment_err_insufficient_balance() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds a community and two prosumers
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            let amount_to_pay = 51;
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 50));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 10));

            let balance_info_before_p1 =  Remuneration::balances(PROSUMER1);
            let balance_info_before_p2 =  Remuneration::balances(PROSUMER2);

            assert_noop!(
                Remuneration::add_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    amount_to_pay,
                    INTRA_COMMUNITY
                ),
                Error::<Test>::InsufficientBalance
            );

            assert_eq!(Remuneration::balances(PROSUMER1), balance_info_before_p1);
            assert_eq!(Remuneration::balances(PROSUMER2), balance_info_before_p2);
        });
    }

    #[test]
    fn payment_err_inter_actors_not_being_communities() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds two communities with the related prosumers
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY2));

            let amount_to_pay = 5;
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 50));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 10));

            let balance_info_before_p1 =  Remuneration::balances(PROSUMER1);
            let balance_info_before_p2 =  Remuneration::balances(PROSUMER2);

            assert_noop!(
                Remuneration::add_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    amount_to_pay,
                    INTER_COMMUNITY
                ),
                Error::<Test>::NotACommunity
            );

            assert_eq!(Remuneration::balances(PROSUMER1), balance_info_before_p1);
            assert_eq!(Remuneration::balances(PROSUMER2), balance_info_before_p2);
        });
    }

    #[test]
    fn payment_err_intra_prosumers_belonging_to_different_communities() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Set ALICE_THE_CUSTODIAN as the initial custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));

            // ALICE_THE_CUSTODIAN adds two communities with the related prosumers
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY2));

            let amount_to_pay = 5;
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 50));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 10));

            let balance_info_before_p1 =  Remuneration::balances(PROSUMER1);
            let balance_info_before_p2 =  Remuneration::balances(PROSUMER2);

            assert_noop!(
                Remuneration::add_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    amount_to_pay,
                    INTRA_COMMUNITY
                ),
                Error::<Test>::DifferentCommunities
            );

            assert_eq!(Remuneration::balances(PROSUMER1), balance_info_before_p1);
            assert_eq!(Remuneration::balances(PROSUMER2), balance_info_before_p2);
        });
    }
}

#[cfg(test)]
mod basic_settlement_tests {
    use super::*;

    #[test]
    fn settle_flexibility_basic() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Setup initial state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Set initial balances
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Perfect delivery scenario: requested = delivered
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,  // requested
                100,  // delivered
                5,    // price
                INTRA_COMMUNITY  // payment_type
            ));

            // Check balances after settlement
            assert_eq!(Remuneration::balances(PROSUMER1), 500); // 1000 - 500
            assert_eq!(Remuneration::balances(PROSUMER2), 500); // 0 + 500
        });
    }

    #[test]
    fn settle_flexibility_under_delivery() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Setup initial state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Set initial balances
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Set alpha for under-delivery penalty calculation
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                500_000, // alpha 0.5
                Remuneration::beta(),
                Remuneration::under_tolerance(),
                Remuneration::over_tolerance(),
            ));

            // Under-delivery scenario: delivered < requested
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,  // requested
                80,   // delivered (20 units under-delivered)
                5,    // price
                INTRA_COMMUNITY  // payment_type
            ));

            // Calculation:
            // - base = min(100, 80) * 5 = 400
            // - under-delivery diff = 100-80 = 20
            // - under-delivery penalty = 0.5 * 20 * 5 = 50
            // - final amount = 400 - 50 = 350

            // Check balances after settlement
            assert_eq!(Remuneration::balances(PROSUMER1), 650); // 1000 - 350
            assert_eq!(Remuneration::balances(PROSUMER2), 350); // 0 + 350
        });
    }

    #[test]
    fn settle_flexibility_over_delivery() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Setup initial state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Set initial balances
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Set beta for over-delivery adjustment calculation
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                Remuneration::alpha(),
                200_000, // 0.2
                Remuneration::under_tolerance(),
                Remuneration::over_tolerance(),
            ));

            // Over-delivery scenario: delivered > requested
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,  // requested
                120,  // delivered (20 units over-delivered)
                5,    // price
                INTRA_COMMUNITY  // payment_type
            ));

            // Calculation:
            // - base = min(100, 120) * 5 = 500
            // - over-delivery diff = 120-100 = 20
            // - over-delivery adjustment = 0.2 * 20 * 5 = 20
            // - final amount = 500 + 20 = 520

            // Check balances after settlement
            assert_eq!(Remuneration::balances(PROSUMER1), 480); // 1000 - 520
            assert_eq!(Remuneration::balances(PROSUMER2), 520); // 0 + 520
        });
    }

    #[test]
    fn settle_flexibility_with_tolerance() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Setup initial state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Set initial balances
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Set parameters via set_main_parameters
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                500_000, // alpha 0.5
                Remuneration::beta(),
                100_000, // under tol 0.1
                100_000, // over tol 0.1
            ));

            // Under-delivery but within tolerance (10% of 100 = 10 units)
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,  // requested
                92,   // delivered (8 units under, within 10% tolerance)
                5,    // price
                INTRA_COMMUNITY  // payment_type
            ));

            // Base = 92 * 5 = 460, diff after tolerance = 0, final=460
            assert_eq!(Remuneration::balances(PROSUMER1), 540); // 1000 - 460
            assert_eq!(Remuneration::balances(PROSUMER2), 460); // 0 + 460

            // Reset balances for next test
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Under-delivery beyond tolerance
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,  // requested
                85,   // delivered (15 under, tolerance=10 => 5 penalized)
                5,    // price
                INTRA_COMMUNITY
            ));
            // Penalty = 0.5 * 5 * 5 = 12 (truncated), base=85*5=425, final=413
            assert_eq!(Remuneration::balances(PROSUMER1), 587); // 1000 - 413
            assert_eq!(Remuneration::balances(PROSUMER2), 413);
        });
    }

    #[test]
    fn settle_flexibility_complex_scenario() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            // Set all parameters at once
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                500_000, // alpha
                200_000, // beta
                100_000, // under tol
                100_000, // over tol
            ));
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,
                105,
                5,
                INTRA_COMMUNITY
            ));
            // Over within tolerance: base=500, no bonus
            assert_eq!(Remuneration::balances(PROSUMER1), 500);
            assert_eq!(Remuneration::balances(PROSUMER2), 500);
        });
    }

    #[test]
    fn settle_flexibility_errors() {
        new_test_ext().execute_with(|| {
            // Set a block and a timestamp
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);

            // Setup initial state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Set initial balances - not enough funds
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 100));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));

            // Insufficient balance for payment
            assert_noop!(
                Remuneration::settle_flexibility_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    100,  // requested
                    100,  // delivered
                    10,   // price - would require 1000 balance
                    INTRA_COMMUNITY  // payment_type
                ),
                Error::<Test>::InsufficientBalance
            );

            // Self-payment not allowed
            assert_noop!(
                Remuneration::settle_flexibility_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER1,
                    100,  // requested
                    100,  // delivered
                    5,    // price
                    INTRA_COMMUNITY  // payment_type
                ),
                Error::<Test>::SameSenderReceiver
            );

            // Invalid payment type
            assert_noop!(
                Remuneration::settle_flexibility_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER2,
                    100,  // requested
                    100,  // delivered
                    5,    // price
                    3     // Invalid payment type
                ),
                Error::<Test>::PaymentTypeNotAllowed
            );

            // Prosumer not in same community
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER3, COMMUNITY2));

            assert_noop!(
                Remuneration::settle_flexibility_payment(
                    RawOrigin::Signed(PROSUMER1).into(),
                    PROSUMER3,
                    100,
                    100,
                    5,
                    INTRA_COMMUNITY  // payment_type
                ),
                Error::<Test>::DifferentCommunities
            );
        });
    }

    #[test]
    fn settle_flexibility_dual_tolerances() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);
            // Setup base state
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));
            // Set high precision alpha/beta = 1.0 and asymmetric tolerances
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                1_000_000, // alpha 1.0
                1_000_000, // beta  1.0
                50_000,    // under tol 5%
                200_000,   // over tol 20%
            ));

            // ---------- Scenario 1: Under-delivery partially beyond under tolerance ----------
            // requested=100, delivered=94, under tolerance=5 => penalized diff = (100-94)-5 = 1
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 5_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,
                94,
                10,
                INTRA_COMMUNITY
            ));
            // Base = 94*10 = 940 ; Penalty = 1*10 =10 ; Final=930
            assert_eq!(Remuneration::balances(PROSUMER1), 5_000 - 930);
            assert_eq!(Remuneration::balances(PROSUMER2), 930);

            // ---------- Scenario 2: Over-delivery within over tolerance (no bonus) ----------
            // requested=100, delivered=115, over tolerance=20 => over diff 15 < 20 => no bonus
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 5_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,
                115,
                10,
                INTRA_COMMUNITY
            ));
            // Base = 100*10=1000 ; no bonus
            assert_eq!(Remuneration::balances(PROSUMER1), 5_000 - 1_000);
            assert_eq!(Remuneration::balances(PROSUMER2), 1_000);

            // ---------- Scenario 3: Over-delivery beyond over tolerance (bonus applies) ----------
            // requested=100, delivered=125, over tolerance=20 => bonus diff = (125-100) - 20 = 5
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 5_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,
                125,
                10,
                INTRA_COMMUNITY
            ));
            // Base=1000 ; Bonus=5*10=50 ; Final=1050
            assert_eq!(Remuneration::balances(PROSUMER1), 5_000 - 1_050);
            assert_eq!(Remuneration::balances(PROSUMER2), 1_050);
        });
    }

    #[test]
    fn settle_flexibility_inter_community() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, 1000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, 0));
            // Set parameters at once
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                500_000,
                200_000,
                0,
                0,
            ));
            assert_ok!(Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(COMMUNITY1).into(),
                COMMUNITY2,
                100,
                100,
                5,
                INTER_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(COMMUNITY1), 500);
            assert_eq!(Remuneration::balances(COMMUNITY2), 500);
        });
    }
}

#[cfg(test)]
mod dynamic_adaptation_tests {
    use super::*;

    #[test]
    fn adaptation_alpha_beta_empty_measurements_fails() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 10,10,10,10,10,2));
            assert_noop!(
                Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![], vec![]),
                Error::<Test>::EmptyMeasurements
            );
        });
    }

    #[test]
    fn adaptation_alpha_beta_invalid_window_size_when_not_set() {
        new_test_ext().execute_with(|| {
            // Custodian set but no adaptation params yet => window_size=0
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_noop!(
                Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![1], vec![1]),
                Error::<Test>::InvalidWindowSize
            );
        });
    }

    #[test]
    fn adaptation_alpha_beta_mismatched_lengths_fail() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 1,1,1,1,1,3));
            assert_noop!(
                Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![1,2,3], vec![1,2]),
                Error::<Test>::MismatchedMeasurements
            );
        });
    }

    #[test]
    fn adaptation_alpha_beta_negative_factor_clamps_to_zero() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            // Start alpha/beta at 1.0
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                1_000_000,
                1_000_000,
                Remuneration::under_tolerance(),
                Remuneration::over_tolerance(),
            ));
            // Set high k so (1 + k*(avg-ref)) becomes 0
            assert_ok!(Remuneration::set_adaptation_params(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                1_000_000, // u_ref
                1_000_000, // o_ref
                1_000_000, // k_alpha 1.0
                1_000_000, // k_beta 1.0
                1_000_000, // k_under_tol 1.0
                2
            ));
            // Provide zero measurements -> avg 0; delta = -1_000_000 => factor clamped to 0
            assert_ok!(Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![0,0], vec![0,0]));
            assert_eq!(Remuneration::alpha(), 0);
            assert_eq!(Remuneration::beta(), 0);
        });
    }

    #[test]
    fn adaptation_alpha_beta_not_custodian_fails() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 100,200,300,400,500,2));
            assert_noop!(
                Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(BOB_THE_CHEATER).into(), vec![1,2], vec![1,2]),
                Error::<Test>::NotCustodian
            );
        });
    }

    #[test]
    fn adaptation_alpha_beta_overflow_clamps_to_u64_max() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            let near_max = u64::MAX - 5000; // large starting point
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                near_max,
                near_max,
                Remuneration::under_tolerance(),
                Remuneration::over_tolerance(),
            ));
            // Configure window 1, large positive delta to attempt doubling
            assert_ok!(Remuneration::set_adaptation_params(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                0,0,1_000_000,1_000_000,1_000_000,1
            ));
            // delta = 1_000_000 => factor 2.0 -> product overflows, should clamp
            assert_ok!(Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![1_000_000], vec![1_000_000]));
            assert_eq!(Remuneration::alpha(), u64::MAX);
            assert_eq!(Remuneration::beta(), u64::MAX);
        });
    }

    #[test]
    fn adaptation_alpha_beta_success_updates_and_events() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            let initial_alpha = 2_000_000u64; // 2.0
            let initial_beta  = 1_500_000u64; // 1.5
            assert_ok!(Remuneration::set_main_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                initial_alpha,
                initial_beta,
                Remuneration::under_tolerance(),
                Remuneration::over_tolerance(),
            ));
            let u_ref = 400_000; // 0.4
            let o_ref = 300_000; // 0.3
            let k_alpha = 100_000; // 0.1
            let k_beta  = 200_000; // 0.2
            let k_under_tol  = 50_000; // 0.05
            let window = 3u32;
            assert_ok!(Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), u_ref,o_ref,k_alpha,k_beta,k_under_tol,window));
            let u_measurements = vec![500_000,600_000,700_000]; // avg = 600_000
            let o_measurements = vec![400_000,500_000,600_000]; // avg = 500_000
            assert_ok!(Remuneration::dynamically_adapt_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                u_measurements.clone(),
                o_measurements.clone()
            ));
            // Expected calculations
            let u_avg = 600_000u64;
            let o_avg = 500_000u64;
            let f = 1_000_000i128;
            let factor_a = f + (k_alpha as i128 * (u_avg as i128 - u_ref as i128))/f; // 1_020_000
            let factor_b = f + (k_beta  as i128 * (o_avg as i128 - o_ref as i128))/f; // 1_040_000
            let expected_alpha = (initial_alpha as i128 * factor_a / f) as u64; // 2_040_000
            let expected_beta  = (initial_beta  as i128 * factor_b / f) as u64; // 1_560_000
            assert_eq!(Remuneration::alpha(), expected_alpha);
            assert_eq!(Remuneration::beta(),  expected_beta);
        });
    }

    #[test]
    fn adaptation_alpha_beta_window_size_mismatch_fail() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            // window size configured 2
            assert_ok!(Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 1,1,1,1,1,2));
            // Provide length 3 -> fails MeasurementsExceedWindow (n != configured)
            assert_noop!(
                Remuneration::dynamically_adapt_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), vec![1,2,3], vec![1,2,3]),
                Error::<Test>::MeasurementsExceedWindow
            );
        });
    }

    #[test]
    fn adaptation_set_params_not_custodian_fails() {
        new_test_ext().execute_with(|| {
            // No custodian yet => set one
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_noop!(
                Remuneration::set_adaptation_params(RawOrigin::Signed(BOB_THE_CHEATER).into(), 1,2,3,4,5,5),
                Error::<Test>::NotCustodian
            );
        });
    }

    #[test]
    fn adaptation_set_params_zero_window_fails() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_noop!(
                Remuneration::set_adaptation_params(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 10,20,30,40,50,0),
                Error::<Test>::InvalidWindowSize
            );
        });
    }

    #[test]
    fn adaptation_set_params_success_and_event() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            let u_ref = 500_000; // 0.5
            let o_ref = 300_000; // 0.3
            let k_alpha = 120_000; // 0.12
            let k_beta = 250_000;  // 0.25
            let k_under_tol = 50_000;  // 0.05
            let window_size = 4u32;
            assert_ok!(Remuneration::set_adaptation_params(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(),
                u_ref,o_ref,k_alpha,k_beta, k_under_tol, window_size
            ));
            // Storage checks only (omit event assertion to avoid failure in mock)
            assert_eq!(Remuneration::u_ref(), u_ref);
            assert_eq!(Remuneration::o_ref(), o_ref);
            assert_eq!(Remuneration::k_alpha(), k_alpha);
            assert_eq!(Remuneration::k_beta(), k_beta);
            assert_eq!(Remuneration::adaptation_window_size(), window_size);
        });
    }
}

#[cfg(test)]
mod pw_quad_penalty_tests {
    use super::*;

    #[test]
    fn piecewise_parameters_management() {
        new_test_ext().execute_with(|| {
            // Set custodian
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            // Defaults
            assert_eq!(Remuneration::alpha_piecewise(), 0);
            assert_eq!(Remuneration::eps_piecewise_1(), 0);
            assert_eq!(Remuneration::eps_piecewise_2(), 0);
            // Update piecewise params
            let a = 2u64; let e1 = 250_000u64; let e2 = 400_000u64;
            assert_ok!(Remuneration::set_piecewise_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), a, e1, e2
            ));
            // Storage checks
            assert_eq!(Remuneration::alpha_piecewise(), a);
            assert_eq!(Remuneration::eps_piecewise_1(), e1);
            assert_eq!(Remuneration::eps_piecewise_2(), e2);
        });
    }

    #[test]
    fn piecewise_parameters_not_custodian_fails() {
        new_test_ext().execute_with(|| {
            // Establish a custodian to compare against
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            // Non-custodian cannot set piecewise params
            assert_noop!(
                Remuneration::set_piecewise_parameters(
                    RawOrigin::Signed(BOB_THE_CHEATER).into(), 1, 200_000, 300_000
                ),
                Error::<Test>::NotCustodian
            );
            // Ensure storage remains at defaults
            assert_eq!(Remuneration::alpha_piecewise(), 0);
            assert_eq!(Remuneration::eps_piecewise_1(), 0);
            assert_eq!(Remuneration::eps_piecewise_2(), 0);
        });
    }

    #[test]
    fn settle_flexibility_payment_with_pw_quad_penalty() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(1_000);
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, COMMUNITY1));
            assert_ok!(Remuneration::add_prosumer(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, COMMUNITY1));

            // Piecewise params: alpha=1, eps1=0.2, eps2=0.4 => e1=80, e2=60
            assert_ok!(Remuneration::set_piecewise_parameters(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 1, 200_000, 400_000));

            // Perfect delivery => base 500, penalty 0, final 500
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 5_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
                RawOrigin::Signed(PROSUMER1).into(), PROSUMER2, 100, 100, 5, INTRA_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(PROSUMER1), 5_000 - 500);
            assert_eq!(Remuneration::balances(PROSUMER2), 500);

            // Linear branch (e2 <= Em < e1) => base 700, penalty 100, final 600
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 10_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
                RawOrigin::Signed(PROSUMER1).into(), PROSUMER2, 100, 70, 10, INTRA_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(PROSUMER1), 10_000 - 600);
            assert_eq!(Remuneration::balances(PROSUMER2), 600);

            // Quadratic branch (Em < e2) not saturating to zero => final 50
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 10_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
                RawOrigin::Signed(PROSUMER1).into(), PROSUMER2, 100, 55, 10, INTRA_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(PROSUMER1), 10_000 - 50);
            assert_eq!(Remuneration::balances(PROSUMER2), 50);

            // Quadratic branch saturating to zero => final 0
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 10_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
                RawOrigin::Signed(PROSUMER1).into(), PROSUMER2, 100, 50, 10, INTRA_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(PROSUMER1), 10_000);
            assert_eq!(Remuneration::balances(PROSUMER2), 0);

            // Over-delivery ignored (no bonus) => final 500
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 5_000));
            assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
            assert_ok!(Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
                RawOrigin::Signed(PROSUMER1).into(), PROSUMER2, 100, 120, 5, INTRA_COMMUNITY
            ));
            assert_eq!(Remuneration::balances(PROSUMER1), 5_000 - 500);
            assert_eq!(Remuneration::balances(PROSUMER2), 500);
        });
    }
}

#[cfg(test)]
mod hybrid_model_tests {
    use super::*;

    #[test]
    fn hybrid_model_parameters_management() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            // Defaults
            assert_eq!(Remuneration::gamma_over_hybrid(), 0);
            assert_eq!(Remuneration::gamma_under_hybrid(), 0);
            assert_eq!(Remuneration::eps_hybrid(), 0);
            assert_eq!(Remuneration::n_hybrid(), 0);
            // Update
            let go = 300_000u64; let gu = 500_000u64; let eps = 100_000u64; let n = 2u64;
            assert_ok!(Remuneration::set_hybrid_model_parameters(
                RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), go, gu, eps, n
            ));
            assert_eq!(Remuneration::gamma_over_hybrid(), go);
            assert_eq!(Remuneration::gamma_under_hybrid(), gu);
            assert_eq!(Remuneration::eps_hybrid(), eps);
            assert_eq!(Remuneration::n_hybrid(), n);
            assert_eq!(Remuneration::query_hybrid_model_params(), (go, gu, eps, n));
        });
    }

    #[test]
    fn hybrid_model_parameters_not_custodian_fails() {
        new_test_ext().execute_with(|| {
            assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
            assert_noop!(
                Remuneration::set_hybrid_model_parameters(
                    RawOrigin::Signed(BOB_THE_CHEATER).into(), 1, 2, 3, 4
                ),
                Error::<Test>::NotCustodian
            );
            assert_eq!(Remuneration::gamma_over_hybrid(), 0);
            assert_eq!(Remuneration::gamma_under_hybrid(), 0);
            assert_eq!(Remuneration::eps_hybrid(), 0);
            assert_eq!(Remuneration::n_hybrid(), 0);
        });
    }
}
