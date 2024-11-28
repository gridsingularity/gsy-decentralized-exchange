use super::*;

use gsy_collateral::Pallet as GsyCollateral;
use gsy_primitives::{Bid, BidOfferMatch, Offer, OrderComponent, Trade, TradeParameters, Hash, HashT};

use sp_runtime::traits::BlakeTwo256;

pub struct TestOrderbookFunctions;

impl TestOrderbookFunctions {

    pub fn add_user<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
        let _ = GsyCollateral::<T>::add_user(user);
        Ok(())
    }

    pub fn add_matching_engine_operator<T: Config>(user: T::AccountId) -> Result<(), &'static str> {
        let _ = GsyCollateral::<T>::add_matching_engine_operator(user);
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
                market_uuid: 1u64,
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
                market_uuid: 1u64,
                time_slot: block_number,
                creation_time: 1677453190,
                energy,
                energy_rate
            },
        }
    }

    /// Create a trade with filler values.
    pub fn dummy_trade<T: Config>(
        buyer: T::AccountId,
        seller: T::AccountId,
        selected_energy: u64,
        energy_rate: u64
    ) -> Trade<T::AccountId, Hash> {
        let trade_uuid = BlakeTwo256::hash_of(&1);
        let bid = TestOrderbookFunctions::dummy_bid::<T>(buyer.clone(), 1, selected_energy, energy_rate);
        let offer = TestOrderbookFunctions::dummy_offer::<T>(seller.clone(), 2, selected_energy, energy_rate);
        Trade {
            seller,
            buyer,
            market_id: 2,
            time_slot: 5,
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