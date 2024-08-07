use bitflags::bitflags;
use codec::MaxEncodedLen;
use frame_support::{dispatch::DispatchResult, sp_runtime::DispatchError};
use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Copy, Clone, Encode, Decode, Default, MaxEncodedLen, Debug, PartialEq, TypeInfo)]
pub struct VaultInfo<AccountId, Balance, BlockNumber, VaultId> {
	/// The account id of the vault owner.
	pub owner: AccountId,
	/// The id of the vault.
	pub id: VaultId,
	/// The info of the collateral stored in the vault.
	pub collateral: CollateralInfo<Balance, BlockNumber>,
	/// The status of the vault.
	pub status: VaultStatus,
}

#[derive(Copy, Clone, Encode, Decode, Default, MaxEncodedLen, Debug, PartialEq, TypeInfo)]
pub struct CollateralInfo<Balance, BlockNumber> {
	/// The amount of collateral stored in the vault.
	pub amount: Balance,
	/// The time, in block, when the collateral amount has been modified.
	pub deposit_time: BlockNumber,
}

bitflags! {
	/// Vault status flags.
	#[derive(MaxEncodedLen, Encode, Decode, TypeInfo)]
	pub struct VaultStatus: u32 {
		/// The vault is closed.
		const CLOSED = 0b0000_0001;
		/// The vault is frozen.
		const FROZEN = 0b0000_0010;
		/// The vault withdrawals are blocked.
		const WITHDRAWALS_FROZEN = 0b0000_0100;
		/// The vault deposits are blocked.
		const DEPOSITS_FROZEN = 0b0000_1000;
	}
}

impl Default for VaultStatus {
	fn default() -> Self {
		Self::empty()
	}
}

impl VaultStatus {
	#[inline]
	pub fn is_active(&self) -> bool {
		!self.is_inactive()
	}

	#[inline]
	pub fn is_inactive(&self) -> bool {
		self.contains(Self::CLOSED) || self.is_frozen()
	}

	#[inline]
	pub fn is_frozen(&self) -> bool {
		self.contains(Self::FROZEN)
	}

	#[inline]
	pub fn is_closed(&self) -> bool {
		self.contains(Self::CLOSED)
	}

	#[inline]
	pub fn set_closed(&mut self) {
		self.insert(Self::CLOSED)
	}

	#[inline]
	pub fn unclose(&mut self) {
		self.remove(Self::CLOSED)
	}

	#[inline]
	pub fn set_frozen(&mut self) {
		self.insert(Self::FROZEN)
	}

	#[inline]
	pub fn unfreeze(&mut self) {
		self.remove(Self::FROZEN)
	}

	#[inline]
	pub fn stop_deposits(&mut self) {
		self.insert(Self::DEPOSITS_FROZEN)
	}

	#[inline]
	pub fn stop_withdrawals(&mut self) {
		self.insert(Self::WITHDRAWALS_FROZEN)
	}

	#[inline]
	pub fn allow_deposits(&mut self) {
		self.remove(Self::DEPOSITS_FROZEN)
	}

	#[inline]
	pub fn allow_withdrawals(&mut self) {
		self.remove(Self::WITHDRAWALS_FROZEN)
	}

	#[inline]
	pub fn are_withdrawals_allowed(&self) -> bool {
		!self.withdrawals_frozen()
	}

	#[inline]
	pub fn withdrawals_frozen(&self) -> bool {
		self.contains(Self::WITHDRAWALS_FROZEN) || self.is_frozen()
	}

	#[inline]
	pub fn are_deposits_allowed(&self) -> bool {
		!self.deposits_frozen()
	}

	#[inline]
	pub fn deposits_frozen(&self) -> bool {
		self.contains(Self::DEPOSITS_FROZEN) || self.is_inactive()
	}
}

pub trait Vault {
	type AccountId;
	type Balance;
	type BlockNumber;
	type VaultId: Clone + PartialEq + Default;

	fn account_id(vault_id: &Self::VaultId) -> Self::AccountId;

	/// Create a new vault for the user.
	fn create(account_id: Self::AccountId) -> Result<Self::VaultId, DispatchError>;

	/// Deposit collateral into the vault.
	fn deposit(
		from: &Self::AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError>;

	/// Withdraw collateral from the vault.
	fn withdraw(
		from: &Self::AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError>;
}

/// VaultWithStatus exposes functions to manage the vault status and its functionality.
pub trait VaultWithStatus: Vault {
	/// Allow deposits to the vault.
	fn allow_deposits(account_id: &Self::AccountId) -> DispatchResult;
	/// Allow withdrawals from the vault.
	fn allow_withdrawals(account_id: &Self::AccountId) -> DispatchResult;
	/// Indicates if the vault is allowing deposits. If the vault is frozen, closed or if
	/// withdrawals are disabled, this returns `false`.
	fn are_deposits_allowed(account_id: &Self::AccountId) -> Result<bool, DispatchError>;
	/// Indicates if the vault is allowing withdrawals. If the vault is either frozen, or if
	/// withdrawals are disabled, this returns `false`.
	fn are_withdrawals_allowed(account_id: &Self::AccountId) -> Result<bool, DispatchError>;
	/// Close the vault to stop all deposits and just allow withdrawals.
	fn close(account_id: &Self::AccountId) -> DispatchResult;
	/// Freeze the vault to stop all functionality of the vault.
	fn freeze(account_id: &Self::AccountId) -> DispatchResult;
	/// Indicates if the vault has been closed.
	fn is_closed(account_id: &Self::AccountId) -> Result<bool, DispatchError>;
	/// Indicates if the vault has been frozen.
	fn is_frozen(account_id: &Self::AccountId) -> Result<bool, DispatchError>;
	/// Stop deposits from the vault but allow withdrawals.
	fn stop_deposits(account_id: &Self::AccountId) -> DispatchResult;
	/// Stop withdrawals from the vault but allow deposits.
	fn stop_withdrawals(account_id: &Self::AccountId) -> DispatchResult;
	/// Remove the close flag from the vault.
	fn unclose(account_id: &Self::AccountId) -> DispatchResult;
	/// Unfreeze the vault to allow all functionality of the vault.
	fn unfreeze(account_id: &Self::AccountId) -> DispatchResult;
}
