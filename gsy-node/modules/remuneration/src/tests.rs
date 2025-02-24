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
pub const PROSUMER2: AccountId = AccountId::new(*b"01234653135168356825544612432351");

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
