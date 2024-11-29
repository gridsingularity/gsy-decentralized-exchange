use crate as orderbook_registry;
use frame_support::{parameter_types, traits::fungible::Mutate, PalletId};
use frame_support::pallet_prelude::ConstU32;
use frame_system as system;
use gsy_primitives::v0::AccountId;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage
};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		GsyCollateral: gsy_collateral,
		OrderbookRegistry: orderbook_registry,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
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
	type AccountId = AccountId;
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
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

pub const ALICE: AccountId = AccountId::new(*b"01234567890123456789012345678901");
pub const BOB: AccountId = AccountId::new(*b"01234567890203894950392012432351");
pub const CHARLIE: AccountId = AccountId::new(*b"01234653535968356825454652432351");
pub const MIKE: AccountId = AccountId::new(*b"45678901234568356825456789012345");
pub const JOHN: AccountId = AccountId::new(*b"56789012344653535968356890123456");


impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = u128;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

parameter_types! {
	pub const ProxyAccountLimit: u32 = 3;
	pub const TestPalletID: PalletId = PalletId(*b"test____");
}

impl gsy_collateral::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = TestPalletID;
	type ProxyAccountLimit = ProxyAccountLimit;
	type VaultId = u64;

	type WeightInfo = gsy_collateral::weights::SubstrateWeightInfo<Test>;
}

impl orderbook_registry::Config for Test {

	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type RegistryProxyAccountLimit = ConstU32<3>;
	type WeightInfo = orderbook_registry::weights::SubstrateWeight<Test>;
	type TimeProvider = pallet_timestamp::Pallet<Test>;
}


// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		Balances::set_balance(&ALICE, dollar(1000));
		System::set_block_number(0)
	});
	ext
}

pub fn dollar(d: u128) -> u128 {
	d.saturating_mul(10_u128.pow(12))
}
