use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo)]
pub enum OrderStatus {
	/// Default status,
	Open,
	/// The order has been executed.
	Executed,
	/// The order has been cancelled.
	Deleted,
}

impl Default for OrderStatus {
	fn default() -> Self {
		Self::Open
	}
}

#[derive(Debug, Encode, Decode, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, TypeInfo)]
pub struct OrderReference<AccountId, Hash> {
	// The account id of the user who created the order.
	pub user_id: AccountId,
	// The order reference.
	pub hash: Hash,
}
