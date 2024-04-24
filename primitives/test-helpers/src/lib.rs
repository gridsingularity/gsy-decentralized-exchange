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

//! A set of test helpers to aid in crating meaningful test cases.

use gsy_primitives::v0::{AccountId, Bid, Hash, Offer, OrderComponent, Trade, TradeParameters};
use rand::Rng;
use sp_runtime::traits::{BlakeTwo256, Hash as HashT};

/// Create a bid with filler values.
pub fn dummy_bid(buyer: AccountId, energy: u64, energy_rate: u64) -> Bid<AccountId> {
	Bid {
		buyer,
		nonce: rand::thread_rng().gen_range(1..101),
		bid_component: OrderComponent {
			area_uuid: 1,
			market_uuid: rand::thread_rng().gen_range(1..101),
			time_slot: rand::thread_rng().gen_range(1..101),
			creation_time: 1677453190,
			energy,
			energy_rate
		},
	}
}

/// Create an offer with filler values.
pub fn dummy_offer(
	seller: AccountId,
	energy: u64,
	energy_rate: u64,
) -> Offer<AccountId> {
	Offer {
		seller,
		nonce: rand::thread_rng().gen_range(1..101),
		offer_component: OrderComponent {
			area_uuid: 2,
			market_uuid: rand::thread_rng().gen_range(1..101),
			time_slot: rand::thread_rng().gen_range(1..101),
			creation_time: 1677453190,
			energy,
			energy_rate
		},
	}
}

/// Create a trade with filler values.
pub fn dummy_trade(
	buyer: AccountId,
	seller: AccountId,
	selected_energy: u64,
	energy_rate: u64
) -> Trade<AccountId, Hash> {
	let trade_uuid = BlakeTwo256::hash_of(&rand::thread_rng().gen::<u128>());
	let bid = dummy_bid(buyer.clone(), selected_energy, energy_rate);
	let offer = dummy_offer(seller.clone(), selected_energy, energy_rate);
	Trade {
		seller,
		buyer,
		market_id: rand::thread_rng().gen_range(1..101),
		time_slot: rand::thread_rng().gen_range(1..101),
		trade_uuid,
		creation_time: 1677453190,
		bid: bid.clone(),
		bid_hash: BlakeTwo256::hash_of(&bid),
		offer: offer.clone(),
		offer_hash: BlakeTwo256::hash_of(&offer),
		residual_offer: None,
		residual_bid: None,
		parameters: TradeParameters {
			selected_energy,
			energy_rate,
			trade_uuid,
		}
	}
}
