use crate as trades_settlement;
use frame_support::{PalletId, parameter_types};
use frame_system as system;
use gsy_primitives::v0::{AccountId, Signature};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentityLookup, Verify},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		GsyCollateral: gsy_collateral::{Pallet, Call, Storage, Event<T>},
		OrderbookRegistry: orderbook_registry::{Pallet, Call, Storage, Event<T>},
		OrderbookWorker: orderbook_worker::{Pallet, Call, Storage, Event<T>},
		TradesSettlement: trades_settlement::{Pallet, Call, Storage, Event<T>},
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
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
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

pub type BlockNumber = u64;

pub const ALICE: AccountId = AccountId::new(*b"01234567890123456789012345678901");
pub const BOB: AccountId = AccountId::new(*b"01234567890203894950392012432351");
pub const CHARLIE: AccountId = AccountId::new(*b"01234653535968356825454652432351");
pub const MIKE: AccountId = AccountId::new(*b"45678901234568356825456789012345");

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
	/// The type for recording an account's balance.
	type Balance = u128;
	type DustRemoval = ();
	/// The ubiquitous event type.
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
}

parameter_types! {
	pub const ProxyAccountLimit: u32 = 3;
	pub const TestPalletID: PalletId = PalletId(*b"test____");
}

impl gsy_collateral::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type ProxyAccountLimit = ProxyAccountLimit;
	type PalletId = TestPalletID;
	type VaultId = u64;
	type WeightInfo = gsy_collateral::weights::SubstrateWeightInfo<Test>;
}
impl orderbook_registry::Config for Test {
	type Event = Event;
	type Currency = Balances;
}

impl trades_settlement::Config for Test {
	type Event = Event;
	type WeightInfo = trades_settlement::weights::SubstrateWeightInfo<Test>;
}

parameter_types! {
	// Priority for a transaction. Additive. Higher is better.
	pub const UnsignedPriority: u64 = 1 << 20;
}
impl orderbook_worker::Config for Test {
	type AuthorityId = orderbook_worker::crypto::TestAuthId;
	type Event = Event;
	type Call = Call;
	type UnsignedPriority = UnsignedPriority;
	type WeightInfo = orderbook_worker::weights::SubstrateWeightInfo<Test>;
}

type Extrinsic = TestXt<Call, ()>;

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	type Extrinsic = Extrinsic;
	type OverarchingCall = Call;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(Call, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
