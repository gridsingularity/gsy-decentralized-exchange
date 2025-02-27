use super::*;

use gsy_collateral::Pallet as GsyCollateral;
use gsy_primitives::{Bid, BidOfferMatch, Offer, OrderComponent};

pub struct TestOrderbookFunctions;

impl TestOrderbookFunctions {

    pub fn add_user<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
        let _ = GsyCollateral::<T>::add_user(user);
        Ok(())
    }

    pub fn add_exchange_operator<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
        let _ = GsyCollateral::<T>::add_exchange_operator(user);
        Ok(())
    }

    pub fn dummy_bid<T: Config>(
        buyer: T::AccountId,
        block_number: u64,
        energy: u64,
        energy_rate: u64,
    ) -> Bid<T::AccountId> {
        Bid {
            buyer,
            nonce: 1,
            bid_component: OrderComponent {
                area_uuid: 1,
                market_id: 1u64,
                time_slot: block_number,
                creation_time: 1677453190,
                energy,
                energy_rate
            },
        }
    }

    pub fn dummy_offer<T: Config>(
        seller: T::AccountId,
        block_number: u64,
        energy: u64,
        energy_rate: u64,
    ) -> Offer<T::AccountId> {
        Offer {
            seller,
            nonce: 1,
            offer_component: OrderComponent {
                area_uuid: 2,
                market_id: 1u64,
                time_slot: block_number,
                creation_time: 1677453190,
                energy,
                energy_rate
            },
        }
    }

    pub fn dummy_bid_offer_match<T: Config>(
        bid: Bid<T::AccountId>,
        offer: Offer<T::AccountId>,
        residual_bid: Option<Bid<T::AccountId>>,
        residual_offer: Option<Offer<T::AccountId>>,
        block_number: u64,
        selected_energy: u64,
        energy_rate: u64,
    ) -> BidOfferMatch<T::AccountId> {
        BidOfferMatch {
            market_id: 1,
            time_slot: block_number,
            bid,
            offer,
            residual_offer,
            residual_bid,
            selected_energy,
            energy_rate,
        }
    }
}