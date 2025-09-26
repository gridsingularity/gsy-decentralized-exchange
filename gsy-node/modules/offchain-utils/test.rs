use codec::Decode;
use frame_support::{parameter_types};
use frame_system as system;
use sp_core::{H256, offchain::{testing, OffchainWorkerExt, TransactionPoolExt}};
use sp_runtime::{traits::{BlakeTwo256, IdentityLookup}, BuildStorage};

use offchain_utils as pallet_offchain_utils;

// Minimal mock runtime for testing the pallet's offchain worker

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        OffchainUtils: pallet_offchain_utils,
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
    pub const MaxJobsPerBlockConst: u32 = 10;
}

impl system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeTask = RuntimeTask;
    type Nonce = u64;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
}

impl pallet_offchain_utils::pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxJobsPerBlock = MaxJobsPerBlockConst;
}

fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| system::Pallet::<Test>::set_block_number(1));
    ext
}

#[test]
fn offchain_worker_submits_unsigned_with_expected_float_bits() {
    new_test_ext().execute_with(|| {
        // Set up offchain worker and transaction pool testing extensions
        let (offchain, _offchain_state) = testing::TestOffchainExt::new();
        let (pool, pool_state) = testing::TestTransactionPoolExt::new();
        sp_io::TestExternalities::current().register_extension(OffchainWorkerExt::new(offchain));
        sp_io::TestExternalities::current().register_extension(TransactionPoolExt::new(pool));

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
        let raw_xt = &submitted[0].extrinsic;
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

// Small helper to use assert_ok without importing full sp-runtime testing utils
#[macro_export]
macro_rules! assert_ok {
    ($x:expr) => {{
        match $x {
            Ok(val) => val,
            Err(err) => panic!("Expected Ok(_), got Err({:?})", err),
        }
    }};
}
