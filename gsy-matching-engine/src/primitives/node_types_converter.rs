use gsy_offchain_primitives::types::{Bid, BidOfferMatch, EnergyType, Offer};
use gsy_offchain_primitives::utils::string_to_account_id;
use subxt::utils::{AccountId32, H256};

use crate::connectors::substrate_connector::gsy_node::runtime_types::gsy_primitives::orders::{
    Attributes as NodeAttributes, Bid as NodeBid, EnergyType as NodeEnergyType, Offer as NodeOffer,
    OrderComponent as NodeOrderComponent, Requirements as NodeRequirements,
};

use crate::connectors::substrate_connector::gsy_node::runtime_types::gsy_primitives::trades::BidOfferMatch as NodeBidOfferMatch;

fn create_node_energy_type_from_canonical(energy_type: EnergyType) -> NodeEnergyType {
    match energy_type {
        EnergyType::Battery => NodeEnergyType::Battery,
        EnergyType::Clean => NodeEnergyType::Clean,
        EnergyType::Import => NodeEnergyType::Import,
        EnergyType::FossilFuel => NodeEnergyType::FossilFuel,
    }
}

pub fn create_node_bid_offer_matches_from_canonical(
    matches: Vec<BidOfferMatch>,
) -> Vec<NodeBidOfferMatch<AccountId32, H256>> {
    matches.into_iter().map(|bid_offer_match| {
        crate::connectors::substrate_connector::gsy_node::runtime_types::gsy_primitives::trades::BidOfferMatch {
            market_id: bid_offer_match.market_id,
            time_slot: bid_offer_match.time_slot,
            selected_energy: bid_offer_match.selected_energy,
            energy_rate: bid_offer_match.energy_rate,
            bid: create_node_bid_from_canonical_bid(bid_offer_match.bid),
            offer: create_node_offer_from_canonical_offer(bid_offer_match.offer),
            residual_bid: bid_offer_match.residual_bid.map(|b| create_node_bid_from_canonical_bid(b)),
            residual_offer: bid_offer_match.residual_offer.map(|o| create_node_offer_from_canonical_offer(o)),
        }
    }).collect()
}

fn create_node_offer_from_canonical_offer(offer: Offer) -> NodeOffer<AccountId32> {
    NodeOffer {
        seller: string_to_account_id(offer.seller.to_string()).unwrap(),
        nonce: offer.nonce,
        offer_component: NodeOrderComponent {
            area_uuid: offer.offer_component.area_uuid,
            market_id: offer.offer_component.market_id,
            time_slot: offer.offer_component.time_slot,
            creation_time: offer.offer_component.creation_time,
            energy: offer.offer_component.energy,
            energy_rate: offer.offer_component.energy_rate,
        },
        attributes: offer.attributes.map(|attr| NodeAttributes {
            trading_partner_id: attr.trading_partner_id,
            energy_type: create_node_energy_type_from_canonical(attr.energy_type),
        }),
    }
}

fn create_node_bid_from_canonical_bid(bid: Bid) -> NodeBid<AccountId32> {
    NodeBid {
        buyer: bid.buyer,
        nonce: bid.nonce,
        bid_component: NodeOrderComponent {
            area_uuid: bid.bid_component.area_uuid,
            market_id: bid.bid_component.market_id,
            time_slot: bid.bid_component.time_slot,
            creation_time: bid.bid_component.creation_time,
            energy: bid.bid_component.energy,
            energy_rate: bid.bid_component.energy_rate,
        },
        requirements: bid.requirements.map(|req| NodeRequirements {
            trading_partner_id: req
                .trading_partner_id
                .map(|id| string_to_account_id(id.to_string()).unwrap()),
            energy_type: req
                .energy_type
                .map(|r| create_node_energy_type_from_canonical(r)),
            preferred_energy_rate: req.preferred_energy_rate,
        }),
    }
}
