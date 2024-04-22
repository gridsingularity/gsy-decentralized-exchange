// This file is part of GSy-Decentralized Energy Exchange.

// Copyright (C) Grid Singularity Gmbh.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

//! Core GSy-DEEX types.
//!
//! These core GSy-DEEX types are used by the node modules.

use sp_runtime::{
	generic,
	traits::{IdentifyAccount, Verify},
	MultiSignature,
};

pub use sp_runtime::traits::{BlakeTwo256, Hash as HashT};

/// The block number type used by GSy-node.
pub type BlockNumber = u64;

/// An instant or duration in time.
pub type Moment = u64;

/// Alias to type for a signature for a transaction on the GSy chain. This allows one of several
/// kinds of underlying crypto to be used, so isn't a fixed size when encoded.
pub type Signature = MultiSignature;

/// Alias to the public key used for the GSy chain, actually a `MultiSigner`. Like the signature, this
/// also isn't a fixed size when encoded, as different cryptos have different size public keys.
pub type AccountPublic = <Signature as Verify>::Signer;

/// Alias to the opaque account ID type for the GSy chain, actually a `AccountId32`. This is always
/// 32 bytes.
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

/// The type for looking up accounts.
pub type AccountIndex = u32;

/// Identifier for a chain.
pub type ChainId = u32;

/// A hash of some data used by the GSy chain.
pub type Hash = sp_core::H256;

/// Index of a transaction in the relay chain. 32-bit should be plenty.
pub type Nonce = u32;

/// The balance of an account.
/// 128-bits (or 38 significant decimal figures) will allow for 10 m currency (`10^7`) at a resolution
/// to all for one second's worth of an annualised 50% reward be paid to a unit holder (`10^11` unit
/// denomination), or `10^18` total atomic units, to grow at 50%/year for 51 years (`10^9` multiplier)
/// for an eventual total of `10^27` units (27 significant decimal figures).
/// We round denomination to `10^12` (12 SDF), and leave the other redundancy at the upper end so
/// that 32 bits may be multiplied with a balance in 128 bits without worrying about overflow.
pub type Balance = u128;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// Block ID.
pub type BlockId = generic::BlockId<Block>;
/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

mod orders;
pub use orders::{OrderReference, OrderStatus};

/// `V0` primitives.
pub mod v0 {
	pub use super::*;
}
