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

//! # Trades Settlement ( trades-settlement )
//!
//!
//! A trades settlement system is a system that manages the settlement of  the trades executed
//! within the GSy-Decentralized Energy Exchange. This module allows the registered matching engine
//! (Matching Engine) to add the trade structs for the orders inserted by the users into the
//! GSy-Decentralized Energy Exchange. Moreover, it verifies the correctness of the matched trades
//! and updates the orders status and the involved structures after the trade execution.

#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::weights::TradeSettlementWeightInfo;
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_orders;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::TradeSettlementWeightInfo;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*, dispatch::RawOrigin};
	use frame_support::{sp_runtime::traits::Hash, transactional};
	use frame_system::{ensure_signed, pallet_prelude::*};
	use scale_info::prelude::vec::Vec;
	use sp_std::vec;
	use gsy_primitives::v0::{Bid, BidOfferMatch, Offer, Order, OrderComponent, Validator};

	#[pallet::config]
	pub trait Config:
		frame_system::Config + orderbook_registry::Config + orderbook_worker::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type TradeSettlementWeightInfo: TradeSettlementWeightInfo;

		/// The length of the market slot in seconds.
		#[pallet::constant]
		type MarketSlotDuration: Get<u64>;
	}

	#[pallet::pallet]
	// #[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		TradeCleared(u8, u8),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Ensure that bid, offer and trade vectors have the same length.
		NotSameLength,
		/// Ensure that there are valid matches to settle.
		NoValidMatchToSettle,
		/// Ensure that the orders execution returned an Ok(()) value.
		OrdersNotExecutable,
		/// Ensure the order has been registered in the orderbook registry.
		OrdersNotRegistered,
		/// Ensure that the offered energy rate is lower than the bid energy rate.
		OfferEnergyRateGreaterThanBidEnergyRate,
		/// Ensure that the offered energy rate is higher than the  selected energy.
		OfferEnergyLessThanSelectedEnergy,
		/// Ensure that the bid energy rate is higher than the  selected energy.
		BidEnergyLessThanSelectedEnergy,
		/// Ensure that the energy subtraction in the validation is correct.
		UnableToSubtractEnergy,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Verify the recommended trade matches
		///
		/// # Parameters
		/// `origin`: The origin of the extrinsic. The Matching Engine operator who wants to settle the matches.
		/// `proposed_matches`: Vector of BidOfferMatch structures. Recommended matches for potential trades.
		#[transactional]
		#[pallet::weight(< T as Config >::TradeSettlementWeightInfo::settle_trades())]
		#[pallet::call_index(0)]
		pub fn settle_trades(
			origin: OriginFor<T>,
			proposed_matches: Vec<BidOfferMatch<T::AccountId>>,
		) -> DispatchResult {
			let matching_engine_operator = ensure_signed(origin)?;

			let valid_matches: Vec<_> = proposed_matches
				.into_iter()
				.filter(|bid_offer_match| <Self as Validator>::validate(bid_offer_match))
				.collect();

			if valid_matches.len() > 0 {
				for valid_match in valid_matches.clone() {
					// Check residual orders and add them to storage.
					if let Some(residual_bid) = valid_match.residual_bid {
						// Add residual bid in the orderbook registry.
						<orderbook_registry::Pallet<T>>::insert_orders(
							RawOrigin::Signed(residual_bid.buyer.clone()).into(),
							vec![T::Hashing::hash_of(&Order::Bid(residual_bid.clone()))],
						)?;
						// Add residual in the orderbook worker.
						<orderbook_worker::Pallet<T>>::add_order(
							residual_bid.buyer.clone(),
							Order::Bid(residual_bid),
						)?;
					}
					if let Some(residual_offer) = valid_match.residual_offer {
						// Add residual in the orderbook registry.
						<orderbook_registry::Pallet<T>>::insert_orders(
							RawOrigin::Signed(residual_offer.seller.clone()).into(),
							vec![T::Hashing::hash_of(&Order::Offer(residual_offer.clone()))],
						)?;
						// Add residual in the orderbook worker.
						<orderbook_worker::Pallet<T>>::add_order(
							residual_offer.seller.clone(),
							Order::Offer(residual_offer),
						)?;
					}
				}

				<orderbook_registry::Pallet<T>>::clear_orders_batch(matching_engine_operator, valid_matches)?;

				Ok(())
			} else {
				Err(Error::<T>::NoValidMatchToSettle.into())
			}
		}
	}

	impl<T: Config> Validator for Pallet<T> {
		type AccountId = T::AccountId;

		fn validate(bid_offer_match: &BidOfferMatch<Self::AccountId>) -> bool {
			if !Self::validate_bid_energy_component(
				bid_offer_match.bid.bid_component.energy,
				bid_offer_match.selected_energy,
			) || !Self::validate_offer_energy_component(
				bid_offer_match.offer.offer_component.energy,
				bid_offer_match.selected_energy,
			) || !Self::validate_energy_rate(
				bid_offer_match.bid.bid_component.energy_rate,
				bid_offer_match.offer.offer_component.energy_rate,
			) || !Self::validate_time_slots(
				bid_offer_match
					.bid
					.bid_component
					.time_slot
					.checked_div(T::MarketSlotDuration::get())
					.unwrap_or(0),
				bid_offer_match
					.offer
					.offer_component
					.time_slot
					.checked_div(T::MarketSlotDuration::get())
					.unwrap_or(0),
				// T::TimeProvider::now()
				// 	.as_secs()
				// 	.checked_div(T::MarketSlotDuration::get())
				// 	.unwrap_or(0),
				bid_offer_match.time_slot.checked_div(T::MarketSlotDuration::get()).unwrap_or(0),
			) {
				return false;
			}
			match (bid_offer_match.residual_offer.clone(), bid_offer_match.residual_bid.clone()) {
				(Some(residual_offer), Some(residual_bid)) => {
					if !Self::validate_residual_bid(
						&residual_bid,
						&bid_offer_match.bid,
						bid_offer_match.selected_energy,
					) || !Self::validate_residual_offer(
						&residual_offer,
						&bid_offer_match.offer,
						bid_offer_match.selected_energy,
					) {
						return false;
					}
				},
				(Some(residual_offer), None) => {
					if !Self::validate_residual_offer(
						&residual_offer,
						&bid_offer_match.offer,
						bid_offer_match.selected_energy,
					) {
						return false;
					}
				},
				(None, Some(residual_bid)) => {
					if !Self::validate_residual_bid(
						&residual_bid,
						&bid_offer_match.bid,
						bid_offer_match.selected_energy,
					) {
						return false;
					}
				},
				(None, None) => {},
			}
			true
		}

		fn validate_bid_energy_component(bid_component_energy: u64, selected_energy: u64) -> bool {
			bid_component_energy >= selected_energy
		}

		fn validate_offer_energy_component(
			offer_component_energy: u64,
			selected_energy: u64,
		) -> bool {
			offer_component_energy >= selected_energy
		}

		fn validate_energy_rate(bid_energy_rate: u64, offer_energy_rate: u64) -> bool {
			bid_energy_rate >= offer_energy_rate
		}

		fn validate_residual_bid(
			residual_bid: &Bid<Self::AccountId>,
			bid: &Bid<Self::AccountId>,
			selected_energy: u64,
		) -> bool {
			residual_bid.eq(&Bid {
				nonce: bid.nonce.clone().checked_add(1).unwrap(),
				bid_component: OrderComponent {
					energy: (bid.bid_component.energy.checked_sub(selected_energy).unwrap()).into(),
					..bid.bid_component.clone()
				},
				..bid.clone()
			})
		}

		fn validate_residual_offer(
			residual_offer: &Offer<Self::AccountId>,
			offer: &Offer<Self::AccountId>,
			selected_energy: u64,
		) -> bool {
			residual_offer.eq(&Offer {
				nonce: offer.nonce.clone().checked_add(1).unwrap(),
				offer_component: OrderComponent {
					energy: (offer.offer_component.energy.checked_sub(selected_energy).unwrap())
						.into(),
					..offer.offer_component.clone()
				},
				..offer.clone()
			})
		}

		fn validate_time_slots(
			bid_market_slot: u64,
			offer_market_slot: u64,
			proposed_match_market_slot: u64,
		) -> bool {
			offer_market_slot == bid_market_slot
				&& proposed_match_market_slot == offer_market_slot
		}
	}
}
