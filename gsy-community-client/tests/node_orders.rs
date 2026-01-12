use gsy_community_client::node_connector::orders::create_input_orders;
use gsy_community_client::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::InputOrder;
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::h256_to_string;
use subxt::utils::H256;
use subxt_signer::sr25519::dev;

#[cfg(test)]
mod tests {
    use super::*;
    use gsy_offchain_primitives::db_api_schema::market::AssetType;
    use tracing::Level;
    use tracing_subscriber;

    fn setup_tracing() {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }

    #[test]
    fn test_orders_to_node_are_created_correctly() {
        setup_tracing();
        let area_hash_1 = h256_to_string(H256::random());
        let area_hash_2 = h256_to_string(H256::random());
        let forecasts: Vec<ForecastSchema> = vec![
            ForecastSchema {
                area_uuid: "area1".to_string(),
                area_hash: area_hash_1.clone(),
                creation_time: 123123,
                time_slot: 456456,
                energy_kwh: 12.,
                community_uuid: "community1".to_string(),
                confidence: 0.8,
            },
            ForecastSchema {
                area_uuid: "area2".to_string(),
                area_hash: area_hash_2.clone(),
                creation_time: 234234,
                time_slot: 456456,
                energy_kwh: -1.,
                community_uuid: "community1".to_string(),
                confidence: 0.1,
            },
        ];

        let market: MarketTopologySchema = MarketTopologySchema {
            creation_time: 345345,
            time_slot: 456456,
            market_id: h256_to_string(H256::random()),
            community_uuid: "community1".to_string(),
            community_name: "My Community".to_string(),
            community_areas: vec![
                AreaTopologySchema {
                    area_uuid: "area1".to_string(),
                    area_type: AssetType::BATTERY,
                    area_hash: area_hash_1.clone(),
                    name: "Area 1".to_string(),
                },
                AreaTopologySchema {
                    area_uuid: "area2".to_string(),
                    area_type: AssetType::BATTERY,
                    area_hash: area_hash_2.clone(),
                    name: "Area 2".to_string(),
                },
            ],
        };

        let input_orders = create_input_orders(forecasts, market.clone(), &dev::alice());
        assert_eq!(input_orders.len(), 2);
        let current_time = get_current_timestamp_in_secs();

        for order in input_orders {
            match (order, market.clone()) {
                (InputOrder::Bid(bid), market) => {
                    let area_info = market.community_areas.get(0).unwrap();
                    assert_eq!(
                        h256_to_string(bid.bid_component.area_uuid),
                        area_info.area_hash
                    );
                    assert_eq!(
                        h256_to_string(bid.bid_component.market_id),
                        market.market_id
                    );
                    assert!((current_time - bid.bid_component.creation_time) < 1);
                    assert_eq!(bid.bid_component.time_slot, 456456);
                    assert_eq!(bid.bid_component.energy_rate, 36000);
                    assert_eq!(bid.bid_component.energy, 120000);
                }
                (InputOrder::Offer(offer), market) => {
                    let area_info = market.community_areas.get(1).unwrap();
                    assert_eq!(
                        h256_to_string(offer.offer_component.area_uuid),
                        area_info.area_hash
                    );
                    assert_eq!(
                        h256_to_string(offer.offer_component.market_id),
                        market.market_id
                    );
                    assert!((current_time - offer.offer_component.creation_time) < 1);
                    assert_eq!(offer.offer_component.time_slot, 456456);
                    assert_eq!(offer.offer_component.energy_rate, 700);
                    assert_eq!(offer.offer_component.energy, 10000);
                }
            }
        }
    }
}
