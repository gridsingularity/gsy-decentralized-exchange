use gsy_offchain_primitives::types::{BidOfferMatch, EnergyType, Order};
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

fn create_node_offer_from_canonical_offer(offer: Order) -> NodeOffer<AccountId32> {
    NodeOffer {
        seller: string_to_account_id(offer.created_by.to_string()).unwrap(),
        nonce: 0,
        offer_component: NodeOrderComponent {
            area_uuid: offer.area_uuid,
            market_id: offer.market_id,
            time_slot: offer.time_slot,
            creation_time: offer.creation_time,
            energy: offer.energy,
            energy_rate: offer.energy_rate,
        },
        attributes: offer.attributes.map(|attr| NodeAttributes {
            trading_partner_id: attr.trading_partner_id,
            energy_type: create_node_energy_type_from_canonical(attr.energy_type),
        }),
    }
}

fn create_node_bid_from_canonical_bid(bid: Order) -> NodeBid<AccountId32> {
    NodeBid {
        buyer: bid.created_by,
        // TODO: Remove the nonce value once the port to Ethereum is completed
        nonce: 0,
        bid_component: NodeOrderComponent {
            area_uuid: bid.area_uuid,
            market_id: bid.market_id,
            time_slot: bid.time_slot,
            creation_time: bid.creation_time,
            energy: bid.energy,
            energy_rate: bid.energy_rate,
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
