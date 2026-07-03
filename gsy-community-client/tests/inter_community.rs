use gsy_community_client::constants::INTER_COMMUNITY_MARKET_NAME;
use gsy_community_client::inter_community::{eligible_inter_community, inter_community_market_id};
use gsy_community_client::node_connector::orders::create_inter_community_order;
use gsy_community_client::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::InputOrder;
use gsy_community_client::offchain_storage_connector::adapter::generate_market_id;
use gsy_offchain_primitives::MarketType;
use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::community_id_from_uuid;
use subxt_signer::sr25519::dev;

#[cfg(test)]
mod tests {
    use super::*;

    const TIME_SLOT: u64 = 456456;

    fn forecast(community_uuid: &str, time_slot: u64, energy_kwh: f64) -> ForecastSchema {
        ForecastSchema {
            area_uuid: "area".to_string(),
            area_hash: "hash".to_string(),
            community_uuid: community_uuid.to_string(),
            time_slot,
            creation_time: 123123,
            energy_kwh,
            confidence: 0.9,
        }
    }

    #[test]
    fn inter_community_market_id_is_deterministic() {
        assert_eq!(
            inter_community_market_id(TIME_SLOT),
            inter_community_market_id(TIME_SLOT)
        );
        assert_ne!(
            inter_community_market_id(TIME_SLOT),
            inter_community_market_id(TIME_SLOT + 900)
        );
        assert_eq!(
            inter_community_market_id(TIME_SLOT),
            generate_market_id(INTER_COMMUNITY_MARKET_NAME, MarketType::Spot, TIME_SLOT)
        );
    }

    #[test]
    fn inter_community_market_id_differs_from_community_spot_ids() {
        for community in ["LugaggiaInnovationCommunity", "GaramèDistrict", "AIC"] {
            assert_ne!(
                inter_community_market_id(TIME_SLOT),
                generate_market_id(community, MarketType::Spot, TIME_SLOT)
            );
        }
    }

    #[test]
    fn eligibility_gate_excludes_non_listed_communities() {
        assert!(eligible_inter_community("LugaggiaInnovationCommunity"));
        assert!(eligible_inter_community("GaramèDistrict"));
        assert!(!eligible_inter_community("SomeOtherCommunity"));
        assert!(!eligible_inter_community(INTER_COMMUNITY_MARKET_NAME));
    }

    #[test]
    fn deficit_creates_single_bid() {
        let community_id = community_id_from_uuid("community-a");
        let market_id = inter_community_market_id(TIME_SLOT);
        let order = create_inter_community_order(
            12.5,
            community_id,
            market_id,
            TIME_SLOT,
            0.3,
            &dev::alice(),
        )
        .expect("deficit must yield a bid");
        match order {
            InputOrder::Bid(bid) => {
                assert_eq!(bid.bid_component.area_uuid, community_id);
                assert_eq!(bid.bid_component.market_id, market_id);
                assert_eq!(bid.bid_component.time_slot, TIME_SLOT);
                assert_eq!(bid.bid_component.energy, 125000);
                assert_eq!(bid.bid_component.energy_rate, 37500);
            }
            InputOrder::Offer(_) => panic!("expected a bid"),
        }
    }

    #[test]
    fn surplus_creates_single_offer() {
        let community_id = community_id_from_uuid("community-b");
        let market_id = inter_community_market_id(TIME_SLOT);
        let order = create_inter_community_order(
            -3.0,
            community_id,
            market_id,
            TIME_SLOT,
            0.07,
            &dev::alice(),
        )
        .expect("surplus must yield an offer");
        match order {
            InputOrder::Offer(offer) => {
                assert_eq!(offer.offer_component.area_uuid, community_id);
                assert_eq!(offer.offer_component.market_id, market_id);
                assert_eq!(offer.offer_component.time_slot, TIME_SLOT);
                assert_eq!(offer.offer_component.energy, 30000);
                assert_eq!(offer.offer_component.energy_rate, 2100);
            }
            InputOrder::Bid(_) => panic!("expected an offer"),
        }
    }

    #[test]
    fn tie_creates_no_order() {
        let community_id = community_id_from_uuid("community-c");
        let market_id = inter_community_market_id(TIME_SLOT);
        for net in [0.0, 1e-10, -1e-10] {
            assert!(
                create_inter_community_order(
                    net,
                    community_id,
                    market_id,
                    TIME_SLOT,
                    0.3,
                    &dev::alice(),
                )
                .is_none()
            );
        }
    }

    #[test]
    fn aggregated_forecasts_yield_one_net_order_per_community() {
        let community_id = community_id_from_uuid("community-d");
        let market_id = inter_community_market_id(TIME_SLOT);
        let forecasts = vec![
            forecast("community-d", TIME_SLOT, 5.0),
            forecast("community-d", TIME_SLOT, 3.0),
            forecast("community-d", TIME_SLOT, -6.5),
            // Other community / other timeslot must not leak into the aggregate.
            forecast("community-e", TIME_SLOT, 100.0),
            forecast("community-d", TIME_SLOT + 900, 100.0),
        ];
        let net = aggregate_net_import(&forecasts, "community-d", TIME_SLOT);
        let order = create_inter_community_order(
            net,
            community_id,
            market_id,
            TIME_SLOT,
            0.3,
            &dev::alice(),
        )
        .expect("net deficit must yield a bid");
        match order {
            InputOrder::Bid(bid) => {
                assert_eq!(bid.bid_component.area_uuid, community_id);
                assert_eq!(bid.bid_component.market_id, market_id);
                assert_eq!(bid.bid_component.energy, 15000);
            }
            InputOrder::Offer(_) => panic!("expected a bid"),
        }
    }
}
