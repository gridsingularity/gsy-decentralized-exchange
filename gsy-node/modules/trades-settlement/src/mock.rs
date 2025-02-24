use crate as trades_settlement;
use frame_support::{PalletId, parameter_types};
use frame_system as system;
use gsy_primitives::v0::{AccountId, Signature};
use sp_core::H256;
use sp_runtime::{
	testing::TestXt,
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentityLookup, Verify},
	BuildStorage
};
pub use pallet_timestamp;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		GsyCollateral: gsy_collateral,
		OrderbookRegistry: orderbook_registry,
		OrderbookWorker: orderbook_worker,
		TradesSettlement: trades_settlement,
		Timestamp: pallet_timestamp,
		Remuneration: remuneration,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
	pub const RemunerationMarketSlotDuration: u64 = 900;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

pub const ALICE: AccountId = AccountId::new(*b"01234567890123456789012345678901");
pub const BOB: AccountId = AccountId::new(*b"01234567890203894950392012432351");
pub const CHARLIE: AccountId = AccountId::new(*b"01234653535968356825454652432351");
pub const MIKE: AccountId = AccountId::new(*b"45678901234568356825456789012345");

parameter_types! {
	pub const MarketSlotDuration: u64 = 900;
	pub const ExistentialDeposit: u128 = 1;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = u128;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

parameter_types! {
	pub const ProxyAccountLimit: u32 = 15;
	pub const TestPalletID: PalletId = PalletId(*b"test____");
}

impl gsy_collateral::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ProxyAccountLimit = ProxyAccountLimit;
	type PalletId = TestPalletID;
	type VaultId = u64;
	type WeightInfo = gsy_collateral::weights::SubstrateWeightInfo<Test>;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

impl orderbook_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type RegistryProxyAccountLimit = ();
	type WeightInfo = ();
	type TimeProvider = pallet_timestamp::Pallet<Test>;
}

impl remuneration::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RemunerationWeightInfo = remuneration::weights::SubstrateWeightInfo<Test>;
	type MarketSlotDuration = RemunerationMarketSlotDuration;
	type RemunerationHandler = Remuneration;
}

impl trades_settlement::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type TradeSettlementWeightInfo = trades_settlement::weights::SubstrateWeightInfo<Test>;
	type MarketSlotDuration = MarketSlotDuration;
	type Remuneration = Remuneration;
}

parameter_types! {
	// Priority for a transaction. Additive. Higher is better.
	pub const UnsignedPriority: u64 = 1 << 20;
}
impl orderbook_worker::Config for Test {
	type AuthorityId = orderbook_worker::crypto::TestAuthId;
	type RuntimeEvent = RuntimeEvent;
	type Call = frame_system::pallet_prelude::RuntimeCallFor<Test>;
	type UnsignedPriority = UnsignedPriority;
	type WeightInfo = orderbook_worker::weights::SubstrateWeightInfo<Test>;
}

type Extrinsic = TestXt<RuntimeCall, ()>;

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type Extrinsic = Extrinsic;
	type OverarchingCall = RuntimeCall;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(RuntimeCall, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
