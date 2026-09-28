use gsy_community_client::node_connector::orders::{calculate_order_rate, create_input_orders};
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
        // Both rates are now precomputed once per market slot by the caller and passed
        // in flat: bids use `bid_rate`, offers `offer_rate`, with no per-forecast
        // recomputation and no dependence on the forecast's confidence.
        let offer_rate = 0.07;
        let input_orders = create_input_orders(
            forecasts,
            market.clone(),
            bid_rate,
            offer_rate,
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
                    // The offer forecast carries confidence 0.1, which no longer affects
                    // price: the total-price energy_rate is just
                    //   1.0 kWh * offer_rate (0.07) * 10000 = 700.
                    assert_eq!(offer.offer_component.energy_rate, 700);
                    assert_eq!(offer.offer_component.energy, 10000);
                }
            }
        }
    }

    /// Two PV offers with wildly different confidences must price identically: the
    /// offer rate is the caller's slot-level ramp value and nothing else. A co-submitted
    /// bid keeps using `bid_rate`.
    #[test]
    fn test_offer_rate_is_independent_of_forecast_confidence() {
        let area_hash_bid = h256_to_string(H256::random());
        let area_hash_offer_hi = h256_to_string(H256::random());
        let area_hash_offer_lo = h256_to_string(H256::random());
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
                area_uuid: "offer_area_hi".to_string(),
                area_hash: area_hash_offer_hi.clone(),
                creation_time: 234234,
                time_slot: 456456,
                energy_kwh: -1.,
                community_uuid: "community1".to_string(),
                confidence: 1.0,
            },
            ForecastSchema {
                area_uuid: "offer_area_lo".to_string(),
                area_hash: area_hash_offer_lo.clone(),
                creation_time: 234234,
                time_slot: 456456,
                energy_kwh: -1.,
                community_uuid: "community1".to_string(),
                confidence: 0.05,
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
                    area_uuid: "offer_area_hi".to_string(),
                    area_type: AssetType::PV,
                    area_hash: area_hash_offer_hi.clone(),
                    name: "Offer Area Hi".to_string(),
                },
                AreaTopologySchema {
                    area_uuid: "offer_area_lo".to_string(),
                    area_type: AssetType::PV,
                    area_hash: area_hash_offer_lo.clone(),
                    name: "Offer Area Lo".to_string(),
                },
            ],
        };

        let bid_rate = 0.3;
        let offer_rate = 0.07;
        let input_orders =
            create_input_orders(forecasts, market.clone(), bid_rate, offer_rate, &dev::alice());
        assert_eq!(input_orders.len(), 3);

        let mut offer_rates = Vec::new();
        for order in input_orders {
            match order {
                InputOrder::Bid(bid) => {
                    // Bids are untouched: 12 * 0.3 * 10000 = 36000.
                    assert_eq!(bid.bid_component.energy_rate, 36000);
                    assert_eq!(bid.bid_component.energy, 120000);
                }
                InputOrder::Offer(offer) => {
                    assert_eq!(offer.offer_component.energy, 10000);
                    offer_rates.push(offer.offer_component.energy_rate);
                }
            }
        }
        // Both offers: 1.0 kWh * 0.07 * 10000 = 700, regardless of confidence.
        assert_eq!(offer_rates, vec![700, 700]);
    }

    /// The offer ramp is the plain default range run backwards: MAX_ORDER_RATE at market
    /// open down to MIN_ORDER_RATE at close, symmetric with the bid ramp. This is what
    /// `main.rs` precomputes and hands to `create_input_orders` as `offer_rate`.
    #[test]
    fn test_offer_ramp_runs_from_max_down_to_min() {
        const MIN: f64 = 0.07;
        const MAX: f64 = 0.30;
        let (open, close) = (1_000u64, 2_000u64);

        let at_open = calculate_order_rate(MIN, MAX, open, open, close, false);
        let midway = calculate_order_rate(MIN, MAX, 1_500, open, close, false);
        let at_close = calculate_order_rate(MIN, MAX, close, open, close, false);

        assert!((at_open - MAX).abs() < 1e-9, "offer opens at MAX, got {at_open}");
        assert!(
            (midway - (MIN + MAX) / 2.0).abs() < 1e-9,
            "offer midpoint should be the band midpoint, got {midway}"
        );
        assert!((at_close - MIN).abs() < 1e-9, "offer closes at MIN, got {at_close}");

        // Mirror image of the bid ramp over the same window.
        assert!(
            (calculate_order_rate(MIN, MAX, open, open, close, true) - MIN).abs() < 1e-9
        );
        assert!(
            (calculate_order_rate(MIN, MAX, close, open, close, true) - MAX).abs() < 1e-9
        );
    }
}
