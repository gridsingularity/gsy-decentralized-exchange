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
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, 500));
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 10));
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, 100));

        let balance_info_before_p1 =  Remuneration::balances(PROSUMER1);
        let balance_info_before_p2 =  Remuneration::balances(PROSUMER2);
        let balance_info_before_c1 =  Remuneration::balances(COMMUNITY1);
        let balance_info_before_c2 =  Remuneration::balances(COMMUNITY2);

        // First case: both sender and receiver are prosumers
        assert_noop!(
            Remuneration::add_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                amount_to_pay,
                INTER_COMMUNITY
            ),
            Error::<Test>::NotACommunity
        );

        // Second case: sender is a community, receiver a prosumer
        assert_noop!(
            Remuneration::add_payment(
                RawOrigin::Signed(COMMUNITY1).into(),
                PROSUMER2,
                amount_to_pay,
                INTER_COMMUNITY
            ),
            Error::<Test>::NotACommunity
        );

        // Third case: sender is a prosumer, receiver a community
        assert_noop!(
            Remuneration::add_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                COMMUNITY2,
                amount_to_pay,
                INTER_COMMUNITY
            ),
            Error::<Test>::NotACommunity
        );

        assert_eq!(Remuneration::balances(PROSUMER1), balance_info_before_p1);
        assert_eq!(Remuneration::balances(PROSUMER2), balance_info_before_p2);
        assert_eq!(Remuneration::balances(COMMUNITY1), balance_info_before_c1);
        assert_eq!(Remuneration::balances(COMMUNITY2), balance_info_before_c2);
    });
}

#[test]
fn update_settlement_parameters() {
    new_test_ext().execute_with(|| {
        // Set ALICE_THE_CUSTODIAN as the initial custodian
        assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
        
        // Default values should be zero
        assert_eq!(Remuneration::alpha(), 0);
        assert_eq!(Remuneration::beta(), 0);
        assert_eq!(Remuneration::tolerance(), 0);
        
        // Update alpha
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5 in fixed point
        assert_eq!(Remuneration::alpha(), 500_000);
        
        // Update beta
        assert_ok!(Remuneration::update_beta(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 200_000)); // 0.2 in fixed point
        assert_eq!(Remuneration::beta(), 200_000);
        
        // Update tolerance
        assert_ok!(Remuneration::update_tolerance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 100_000)); // 0.1 in fixed point
        assert_eq!(Remuneration::tolerance(), 100_000);
        
        // Non-custodian cannot update parameters
        assert_noop!(
            Remuneration::update_alpha(RawOrigin::Signed(BOB_THE_CHEATER).into(), 700_000),
            Error::<Test>::NotCustodian
        );
        assert_noop!(
            Remuneration::update_beta(RawOrigin::Signed(BOB_THE_CHEATER).into(), 300_000),
            Error::<Test>::NotCustodian
        );
        assert_noop!(
            Remuneration::update_tolerance(RawOrigin::Signed(BOB_THE_CHEATER).into(), 150_000),
            Error::<Test>::NotCustodian
        );
    });
}

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
        
        // Parameters are zeros by default, so calculation will be simple
        // With alpha=0, beta=0, tolerance=0:
        // - base = min(requested, delivered) * price = min(100, 100) * 5 = 500
        // - no penalties or bonuses applied
        
        // Perfect delivery scenario: requested = bidded = delivered
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            100, // bidded
            100, // delivered
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 500); // 1000 - 500
        assert_eq!(Remuneration::balances(PROSUMER2), 500); // 0 + 500
        
        // Verify event emission - using proper event assertion pattern
        let flexibility_settled_event = crate::Event::FlexibilitySettled {
            requester: PROSUMER1,
            provider: PROSUMER2,
            requested: 100,
            bidded: 100,
            delivered: 100,
            price: 5,
            calculated_amount: 500,
        };
        System::assert_last_event(flexibility_settled_event.into());
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
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5
        
        // Under-delivery scenario: delivered < requested
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            100, // bidded
            80,  // delivered (20 units under-delivered)
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 80) * 5 = 400
        // - under-delivery diff = 100-80 = 20
        // - under-delivery penalty = 0.5 * 20 * 5 = 50
        // - final amount = 400 - 50 = 350
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 650); // 1000 - 350
        assert_eq!(Remuneration::balances(PROSUMER2), 350); // 0 + 350
        
        // Verify event emission
        let flexibility_settled_event = crate::Event::FlexibilitySettled {
            requester: PROSUMER1,
            provider: PROSUMER2,
            requested: 100,
            bidded: 100,
            delivered: 80,
            price: 5,
            calculated_amount: 350,
        };
        System::assert_last_event(flexibility_settled_event.into());
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
        assert_ok!(Remuneration::update_beta(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 200_000)); // 0.2
        
        // Over-delivery scenario: delivered > requested
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            100, // bidded
            120, // delivered (20 units over-delivered)
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 120) * 5 = 500
        // - over-delivery diff = 120-100 = 20
        // - over-delivery adjustment = 0.2 * 20 * 5 = 20
        // - final amount = 500 + 20 = 520
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 480); // 1000 - 520
        assert_eq!(Remuneration::balances(PROSUMER2), 520); // 0 + 520
        
        // Verify event emission
        let flexibility_settled_event = crate::Event::FlexibilitySettled {
            requester: PROSUMER1,
            provider: PROSUMER2,
            requested: 100,
            bidded: 100,
            delivered: 120,
            price: 5,
            calculated_amount: 520,
        };
        System::assert_last_event(flexibility_settled_event.into());
    });
}

#[test]
fn settle_flexibility_bid_inflation() {
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
        
        // Set alpha for bid inflation penalty calculation
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5
        
        // Bid inflation scenario: bidded > requested
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            120, // bidded (20 units inflated)
            100, // delivered
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 100) * 5 = 500
        // - bid inflation diff = 120-100 = 20
        // - bid inflation penalty = 0.5 * 20 * 5 = 50
        // - final amount = 500 - 50 = 450
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 550); // 1000 - 450
        assert_eq!(Remuneration::balances(PROSUMER2), 450); // 0 + 450
        
        // Verify event emission
        let flexibility_settled_event = crate::Event::FlexibilitySettled {
            requester: PROSUMER1,
            provider: PROSUMER2,
            requested: 100,
            bidded: 120,
            delivered: 100,
            price: 5,
            calculated_amount: 450,
        };
        System::assert_last_event(flexibility_settled_event.into());
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
        
        // Set parameters
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5
        assert_ok!(Remuneration::update_tolerance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 100_000)); // 0.1
        
        // Under-delivery but within tolerance (10% of 100 = 10 units)
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            100, // bidded
            92,  // delivered (8 units under, within 10% tolerance)
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 92) * 5 = 460
        // - threshold = 0.1 * 100 = 10
        // - under-delivery diff = 100-92-10 = 0 (within tolerance)
        // - no penalties apply
        // - final amount = 460
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 540); // 1000 - 460
        assert_eq!(Remuneration::balances(PROSUMER2), 460); // 0 + 460
        
        // Reset balances for next test
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER1, 1000));
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), PROSUMER2, 0));
        
        // Under-delivery beyond tolerance
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            100, // bidded
            85,  // delivered (15 units under, exceeds 10% tolerance)
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 85) * 5 = 425
        // - threshold = 0.1 * 100 = 10
        // - under-delivery diff = 100-85-10 = 5 (beyond tolerance)
        // - under-delivery penalty = 0.5 * 5 * 5 = 12.5 (truncated to 12 in fixed point)
        // - final amount = 425 - 12 = 413
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 587); // 1000 - 413
        assert_eq!(Remuneration::balances(PROSUMER2), 413); // 0 + 413
    });
}

#[test]
fn settle_flexibility_complex_scenario() {
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
        
        // Set all parameters
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5
        assert_ok!(Remuneration::update_beta(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 200_000)); // 0.2
        assert_ok!(Remuneration::update_tolerance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 100_000)); // 0.1
        
        // Combined scenario: slight over-delivery and bid inflation
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(PROSUMER1).into(),
            PROSUMER2,
            100, // requested
            120, // bidded (20 units inflated)
            105, // delivered (5 units over-delivered)
            5,   // price
            INTRA_COMMUNITY
        ));
        
        // Calculation:
        // - base = min(100, 105) * 5 = 500
        // - threshold = 0.1 * 100 = 10
        // - over-delivery diff = 105-100-10 = 0 (within tolerance)
        // - threshold_bid = 0.1 * 120 = 12
        // - bid inflation diff = 120-100-12 = 8 (beyond tolerance)
        // - bid inflation penalty = 0.5 * 8 * 5 = 20
        // - final amount = 500 - 0 + 0 - 20 = 480
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(PROSUMER1), 520); // 1000 - 480
        assert_eq!(Remuneration::balances(PROSUMER2), 480); // 0 + 480
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
                100, // requested
                100, // bidded
                100, // delivered
                10,  // price - would require 1000 balance
                INTRA_COMMUNITY
            ),
            Error::<Test>::InsufficientBalance
        );
        
        // Self-payment not allowed
        assert_noop!(
            Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER1,
                100,
                100,
                100,
                5,
                INTRA_COMMUNITY
            ),
            Error::<Test>::SameSenderReceiver
        );
        
        // Invalid payment type
        assert_noop!(
            Remuneration::settle_flexibility_payment(
                RawOrigin::Signed(PROSUMER1).into(),
                PROSUMER2,
                100,
                100,
                100,
                5,
                3 // Invalid payment type
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
                100,
                5,
                INTRA_COMMUNITY
            ),
            Error::<Test>::DifferentCommunities
        );
    });
}

#[test]
fn settle_flexibility_inter_community() {
    new_test_ext().execute_with(|| {
        // Set a block and a timestamp
        System::set_block_number(1);
        Timestamp::set_timestamp(1_000);
        
        // Setup initial state
        assert_ok!(Remuneration::update_custodian(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), ALICE_THE_CUSTODIAN));
        assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, DSO, COMMUNITY1_OWNER));
        assert_ok!(Remuneration::add_community(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, DSO, COMMUNITY2_OWNER));
        
        // Set initial balances
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY1, 1000));
        assert_ok!(Remuneration::set_balance(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), COMMUNITY2, 0));
        
        // Set parameters
        assert_ok!(Remuneration::update_alpha(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 500_000)); // 0.5
        assert_ok!(Remuneration::update_beta(RawOrigin::Signed(ALICE_THE_CUSTODIAN).into(), 200_000)); // 0.2
        
        // Inter-community flexibility transaction
        assert_ok!(Remuneration::settle_flexibility_payment(
            RawOrigin::Signed(COMMUNITY1).into(),
            COMMUNITY2,
            100, // requested
            100, // bidded
            100, // delivered
            5,   // price
            INTER_COMMUNITY
        ));
        
        // Calculation (exact same formula, but between communities)
        // - base = min(100, 100) * 5 = 500
        // - no penalties apply
        // - final amount = 500
        
        // Check balances after settlement
        assert_eq!(Remuneration::balances(COMMUNITY1), 500); // 1000 - 500
        assert_eq!(Remuneration::balances(COMMUNITY2), 500); // 0 + 500
        
        // Verify event emission
        let flexibility_settled_event = crate::Event::FlexibilitySettled {
            requester: COMMUNITY1,
            provider: COMMUNITY2,
            requested: 100,
            bidded: 100,
            delivered: 100,
            price: 5,
            calculated_amount: 500,
        };
        System::assert_last_event(flexibility_settled_event.into());
    });
}