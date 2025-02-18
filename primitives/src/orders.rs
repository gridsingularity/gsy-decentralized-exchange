use crate::trades::TradeParameters;
use crate::v0::{AccountId, Hash};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{BlakeTwo256, Hash as HashT};

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash))]
pub enum Order<AccountId> {
    Bid(Bid<AccountId>),
    Offer(Offer<AccountId>),
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Hash))]
pub enum InputOrder<AccountId> {
    Bid(InputBid<AccountId>),
    Offer(InputOffer<AccountId>),
}

/// Order component struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct OrderComponent {
    pub area_uuid: Hash,
    pub market_id: Hash,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64,
}

impl OrderComponent {
    /// Compute the blake2-256 hash of the order component.
    pub fn hash(&self) -> Hash {
        BlakeTwo256::hash_of(self)
    }
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct OrderSchema<AccountId, Hash> {
    pub _id: Hash,
    pub status: OrderStatus<Hash>,
    pub order: Order<AccountId>,
}

/// InputBid order struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct InputBid<AccountId> {
    pub buyer: AccountId,
    pub bid_component: OrderComponent,
}

/// Bid order struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct Bid<AccountId> {
    pub buyer: AccountId,
    pub nonce: u32,
    pub bid_component: OrderComponent,
}

impl Bid<AccountId> {
    /// Compute the blake2-256 hash of the Bid order.
    pub fn hash(&self) -> Hash {
        BlakeTwo256::hash_of(self)
    }
}

/// InputOffer (Ask) order struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct InputOffer<AccountId> {
    pub seller: AccountId,
    pub offer_component: OrderComponent,
}

/// Offer (Ask) order struct
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Hash, Default))]
pub struct Offer<AccountId> {
    pub seller: AccountId,
    pub nonce: u32,
    pub offer_component: OrderComponent,
}

impl Offer<AccountId> {
    /// Compute the blake2-256 hash of the Offer order.
    pub fn hash(&self) -> Hash {
        BlakeTwo256::hash_of(self)
    }
}

#[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo)]
pub enum OrderStatus<Hash> {
    /// Default status,
    Open,
    /// The order has been executed.
    Executed(TradeParameters<Hash>),
    /// The order has been cancelled.
    Deleted,
}

impl<Hash> Default for OrderStatus<Hash> {
    fn default() -> Self {
        Self::Open
    }
}

#[derive(Debug, Encode, Decode, Clone, Copy, Eq, PartialEq, Ord, PartialOrd,
    TypeInfo, MaxEncodedLen)]
pub struct OrderReference<AccountId, Hash> {
    // The account id of the user who created the order.
    pub user_id: AccountId,
    // The order reference.
    pub hash: Hash,
}
