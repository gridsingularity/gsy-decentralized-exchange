use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::aggregation::{
    net_to_order_type, aggregate_net_import, OrderType, RESIDUAL_ENERGY_TOLERANCE_KWH,
    CommunityEnergyPoint};

#[cfg(test)]
mod tests {
    use super::*;

    const COMMUNITY: &str = "community-1";
    const OTHER_COMMUNITY: &str = "community-2";
    const TIME_SLOT: u64 = 1_700_000_000;
    const OTHER_TIME_SLOT: u64 = 1_700_000_900;

    fn forecast(community_uuid: &str, time_slot: u64, energy_kwh: f64) -> ForecastSchema {
        ForecastSchema {
            area_uuid: "area".to_string(),
            area_hash: "hash".to_string(),
            community_uuid: community_uuid.to_string(),
            time_slot,
            creation_time: 0,
            energy_kwh,
            confidence: 1.0,
        }
    }

    fn measurement(community_uuid: &str, time_slot: u64, energy_kwh: f64) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: "area".to_string(),
            area_hash: "hash".to_string(),
            community_uuid: community_uuid.to_string(),
            time_slot,
            creation_time: 0,
            energy_kwh,
        }
    }

    fn assert_surplus_yields_offer<T: CommunityEnergyPoint>(points: &[T]) {
        let net = aggregate_net_import(points, COMMUNITY, TIME_SLOT);
        assert!(net < 0.0);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::Offer);
    }

    fn assert_deficit_yields_bid<T: CommunityEnergyPoint>(points: &[T]) {
        let net = aggregate_net_import(points, COMMUNITY, TIME_SLOT);
        assert!(net > 0.0);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::Bid);
    }

    fn assert_tie_yields_none<T: CommunityEnergyPoint>(points: &[T]) {
        let net = aggregate_net_import(points, COMMUNITY, TIME_SLOT);
        assert!(net.abs() <= RESIDUAL_ENERGY_TOLERANCE_KWH);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::None);
    }

    #[test]
    fn surplus_yields_offer_for_both_schemas() {
        // Production (negative) exceeds consumption (positive) → net < 0 → Offer.
        assert_surplus_yields_offer(&[
            forecast(COMMUNITY, TIME_SLOT, 2.0),
            forecast(COMMUNITY, TIME_SLOT, -5.0),
        ]);
        assert_surplus_yields_offer(&[
            measurement(COMMUNITY, TIME_SLOT, 2.0),
            measurement(COMMUNITY, TIME_SLOT, -5.0),
        ]);
    }

    #[test]
    fn deficit_yields_bid_for_both_schemas() {
        // Consumption (positive) exceeds production (negative) → net > 0 → Bid.
        assert_deficit_yields_bid(&[
            forecast(COMMUNITY, TIME_SLOT, 5.0),
            forecast(COMMUNITY, TIME_SLOT, -2.0),
        ]);
        assert_deficit_yields_bid(&[
            measurement(COMMUNITY, TIME_SLOT, 5.0),
            measurement(COMMUNITY, TIME_SLOT, -2.0),
        ]);
    }

    #[test]
    fn exact_tie_yields_none_for_both_schemas() {
        // production == consumption → net within tolerance → no order.
        assert_tie_yields_none(&[
            forecast(COMMUNITY, TIME_SLOT, 3.5),
            forecast(COMMUNITY, TIME_SLOT, -3.5),
        ]);
        assert_tie_yields_none(&[
            measurement(COMMUNITY, TIME_SLOT, 3.5),
            measurement(COMMUNITY, TIME_SLOT, -3.5),
        ]);
    }

    #[test]
    fn mixed_sign_sum_is_signed_total() {
        let forecasts = vec![
            forecast(COMMUNITY, TIME_SLOT, 4.0),
            forecast(COMMUNITY, TIME_SLOT, -1.5),
            forecast(COMMUNITY, TIME_SLOT, 0.5),
            forecast(COMMUNITY, TIME_SLOT, -2.0),
        ];
        let net = aggregate_net_import(&forecasts, COMMUNITY, TIME_SLOT);
        assert!((net - 1.0).abs() <= RESIDUAL_ENERGY_TOLERANCE_KWH);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::Bid);

        let measurements = vec![
            measurement(COMMUNITY, TIME_SLOT, 4.0),
            measurement(COMMUNITY, TIME_SLOT, -1.5),
            measurement(COMMUNITY, TIME_SLOT, 0.5),
            measurement(COMMUNITY, TIME_SLOT, -2.0),
        ];
        let net = aggregate_net_import(&measurements, COMMUNITY, TIME_SLOT);
        assert!((net - 1.0).abs() <= RESIDUAL_ENERGY_TOLERANCE_KWH);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::Bid);
    }

    #[test]
    fn filters_by_community_uuid_and_time_slot() {
        let forecasts = vec![
            forecast(COMMUNITY, TIME_SLOT, 1.0),
            forecast(OTHER_COMMUNITY, TIME_SLOT, 100.0),
            forecast(COMMUNITY, OTHER_TIME_SLOT, -100.0),
            forecast(OTHER_COMMUNITY, OTHER_TIME_SLOT, 100.0),
        ];
        let net = aggregate_net_import(&forecasts, COMMUNITY, TIME_SLOT);
        assert!((net - 1.0).abs() <= RESIDUAL_ENERGY_TOLERANCE_KWH);

        let measurements = vec![
            measurement(COMMUNITY, TIME_SLOT, -1.0),
            measurement(OTHER_COMMUNITY, TIME_SLOT, 100.0),
            measurement(COMMUNITY, OTHER_TIME_SLOT, 100.0),
            measurement(OTHER_COMMUNITY, OTHER_TIME_SLOT, -100.0),
        ];
        let net = aggregate_net_import(&measurements, COMMUNITY, TIME_SLOT);
        assert!((net + 1.0).abs() <= RESIDUAL_ENERGY_TOLERANCE_KWH);
    }

    #[test]
    fn empty_input_yields_zero_net_and_none() {
        let forecasts: Vec<ForecastSchema> = vec![];
        let net = aggregate_net_import(&forecasts, COMMUNITY, TIME_SLOT);
        assert_eq!(net, 0.0);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::None);

        let measurements: Vec<MeasurementSchema> = vec![];
        let net = aggregate_net_import(&measurements, COMMUNITY, TIME_SLOT);
        assert_eq!(net, 0.0);
        assert_eq!(net_to_order_type(net, RESIDUAL_ENERGY_TOLERANCE_KWH), OrderType::None);
    }
}