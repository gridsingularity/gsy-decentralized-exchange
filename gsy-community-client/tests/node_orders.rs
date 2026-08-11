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

        let bid_rate = 0.3;
        // open_time == close_time makes the offer ramp fully progressed, so the offer
        // rate equals its (confidence-modulated) floor deterministically, independent of
        // wall-clock `now`. Bids still use the flat `bid_rate`.
        let (open_time, close_time) = (0u64, 0u64);
        let input_orders = create_input_orders(
            forecasts,
            market.clone(),
            bid_rate,
            open_time,
            close_time,
            &dev::alice(),
        );
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
                    // CHANGED PIN: was 700 (flat MIN_ORDER_RATE 0.07). The offer forecast
                    // carries confidence 0.1, so its rate floor is now lifted:
                    //   effective_min = 0.07 + (1 - 0.1) * 0.5 * (0.30 - 0.07) = 0.1735.
                    // With the ramp fully progressed (open_time == close_time) the offer
                    // rate equals that floor, so the total-price energy_rate is
                    //   1.0 kWh * 0.1735 * 10000 = 1734.9999.. -> 1734 after u64 truncation.
                    assert_eq!(offer.offer_component.energy_rate, 1734);
                    // Energy (committed quantity) is unchanged by the rate lever.
                    assert_eq!(offer.offer_component.energy, 10000);
                }
            }
        }
    }

    /// A full-confidence (1.0) PV offer must reproduce the pre-change offer rate: its
    /// floor stays at MIN_ORDER_RATE (0.07), so with the ramp fully progressed the
    /// total-price energy_rate is 1.0 * 0.07 * 10000 = 700. A co-submitted bid must be
    /// completely unaffected by the offer-only rate lever.
    #[test]
    fn test_full_confidence_offer_reproduces_pre_change_rate() {
        let area_hash_bid = h256_to_string(H256::random());
        let area_hash_offer = h256_to_string(H256::random());
        let forecasts: Vec<ForecastSchema> = vec![
            ForecastSchema {
                area_uuid: "bid_area".to_string(),
                area_hash: area_hash_bid.clone(),
                creation_time: 123123,
                time_slot: 456456,
                energy_kwh: 12.,
                community_uuid: "community1".to_string(),
                confidence: 0.9,
            },
            ForecastSchema {
                area_uuid: "offer_area".to_string(),
                area_hash: area_hash_offer.clone(),
                creation_time: 234234,
                time_slot: 456456,
                energy_kwh: -1.,
                community_uuid: "community1".to_string(),
                confidence: 1.0,
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
                    area_uuid: "bid_area".to_string(),
                    area_type: AssetType::SMART_METER,
                    area_hash: area_hash_bid.clone(),
                    name: "Bid Area".to_string(),
                },
                AreaTopologySchema {
                    area_uuid: "offer_area".to_string(),
                    area_type: AssetType::PV,
                    area_hash: area_hash_offer.clone(),
                    name: "Offer Area".to_string(),
                },
            ],
        };

        let bid_rate = 0.3;
        // Fully-progressed ramp so the offer rate equals its floor deterministically.
        let input_orders =
            create_input_orders(forecasts, market.clone(), bid_rate, 0, 0, &dev::alice());
        assert_eq!(input_orders.len(), 2);

        for order in input_orders {
            match order {
                InputOrder::Bid(bid) => {
                    // Bid is untouched by the offer rate lever: 12 * 0.3 * 10000 = 36000.
                    assert_eq!(bid.bid_component.energy_rate, 36000);
                    assert_eq!(bid.bid_component.energy, 120000);
                }
                InputOrder::Offer(offer) => {
                    // confidence 1.0 -> floor at MIN_ORDER_RATE -> 1.0 * 0.07 * 10000 = 700.
                    assert_eq!(offer.offer_component.energy_rate, 700);
                    assert_eq!(offer.offer_component.energy, 10000);
                }
            }
        }
    }
}
