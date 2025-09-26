use crate as pallet_offchain_utils;
use crate::mock::*;
use codec::Decode;
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use sp_core::offchain::{testing, OffchainWorkerExt, TransactionPoolExt};

#[test]
fn offchain_worker_submits_unsigned_with_expected_float_bits() {
    // Set up offchain worker and transaction pool testing extensions on the externalities
    let (offchain, _offchain_state) = testing::TestOffchainExt::new();
    let (pool, pool_state) = testing::TestTransactionPoolExt::new();

    let mut ext = new_test_ext();
    ext.register_extension(OffchainWorkerExt::new(offchain));
    ext.register_extension(TransactionPoolExt::new(pool));

    ext.execute_with(|| {
        // Create one job: expect res = a/(b+1) + 0.5
        let job_id: u64 = 42;
        let a: u64 = 3;
        let b: u64 = 2; // res = 3/(2+1)+0.5 = 1.5
        assert!(pallet_offchain_utils::pallet::Jobs::<Test>::get(job_id).is_none());
        assert_ok!(OffchainUtils::create_job(RuntimeOrigin::signed(1), job_id, a, b));

        // Run the offchain worker which should compute and submit an unsigned tx
        OffchainUtils::offchain_worker(1);

        // One unsigned extrinsic should be in the pool
        let submitted = pool_state.read().transactions.clone();
        assert_eq!(submitted.len(), 1, "expected exactly one unsigned transaction");

        // Decode the extrinsic and assert it is the expected call with correct bits
        type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
        let raw_xt: &Vec<u8> = &submitted[0];
        let xt: UncheckedExtrinsic = Decode::decode(&mut &raw_xt[..]).expect("decode extrinsic");

        // Expected bits for 1.5f64
        let expected_bits = (1.5f64).to_bits();

        match xt.function {
            RuntimeCall::OffchainUtils(pallet_offchain_utils::pallet::Call::submit_result_unsigned { job_id: jid, result_bits }) => {
                assert_eq!(jid, job_id);
                assert_eq!(result_bits, expected_bits);

                // Dispatch the call to apply state changes and verify storage updates
                assert_ok!(OffchainUtils::submit_result_unsigned(RuntimeOrigin::none(), jid, result_bits));
                assert_eq!(pallet_offchain_utils::pallet::Results::<Test>::get(jid), Some(result_bits));
                assert!(pallet_offchain_utils::pallet::Jobs::<Test>::get(jid).is_none());
            }
            other => panic!("unexpected call submitted by OCW: {:?}", other),
        }
    });
}
