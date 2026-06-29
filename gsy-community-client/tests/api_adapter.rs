use gsy_community_client::external_api::{
    ExternalAreaTopology, ExternalCommunityTopology, ExternalForecast, ExternalMeasurement,
};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::get_last_and_next_timeslot;
use gsy_offchain_primitives::db_api_schema::market::AreaTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::MarketType;

use httpmock::prelude::*;
use tracing::Level;
use tracing_subscriber;

fn setup_tracing() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
}

#[tokio::test]
async fn test_get_or_create_market_topology() {
    setup_tracing();
    let server = MockServer::start();

    let (_, time_slot) = get_last_and_next_timeslot();

    let external_topology = ExternalCommunityTopology {
        community_name: "comm_name".to_string(),
        community_uuid: "comm_uuid".to_string(),
        areas: vec![ExternalAreaTopology {
            area_uuid: "area_uuid".to_string(),
            area_name: "area_name".to_string(),
        }],
    };

    let mock_request = server.mock(|when, then| {
        when.method(POST).path("/markets");
        then.status(200).header("content-type", "application/json");
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    let market = adapter
        .get_or_create_market_topology(external_topology, time_slot)
        .await
        .unwrap();
    assert_eq!(market.market_type, MarketType::Spot);
    assert_eq!(market.time_slot, time_slot as u32);
    assert_eq!(market.community_uuid, "comm_uuid");
    assert_eq!(market.community_name, "comm_name");
    assert_eq!(
        market.community_areas,
        vec![AreaTopologySchema {
            area_uuid: "area_uuid".to_string(),
            name: "area_name".to_string(),
            area_type: "Area".to_string(),
        }]
    );
    mock_request.assert();
}

#[tokio::test]
async fn test_forward_forecast_uses_ontology_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST).path("/measurement-points");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries");
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_forecast(vec![ForecastSchema {
            area_uuid: "area_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: 123123,
            creation_time: 456456,
            energy_kwh: 11.,
            confidence: 0.4,
        }])
        .await
        .unwrap();

    measurement_points_request.assert();
    timeseries_request.assert();
}

#[tokio::test]
async fn test_forward_measurement_uses_ontology_profile_endpoints() {
    let server = MockServer::start();

    let measurement_points_request = server.mock(|when, then| {
        when.method(POST).path("/measurement-points");
        then.status(200);
    });
    let timeseries_request = server.mock(|when, then| {
        when.method(POST).path("/timeseries");
        then.status(200);
    });

    let adapter = AreaMarketInfoAdapter::new(Some(server.base_url()));
    adapter
        .forward_measurement(vec![MeasurementSchema {
            area_uuid: "area_uuid".to_string(),
            community_uuid: "comm_uuid".to_string(),
            time_slot: 123123,
            creation_time: 456456,
            energy_kwh: 11.,
        }])
        .await
        .unwrap();

    measurement_points_request.assert();
    timeseries_request.assert();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_forecast_to_internal_schema() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let forecast = ExternalForecast {
            time_slot: 123123,
            creation_time: 456456,
            community_uuid: "comm_uuid".to_string(),
            energy_kwh: 11.,
            area_uuid: "area_uuid".to_string(),
            confidence: 0.4,
        };
        let converted_forecast =
            adapter.convert_forecast_to_internal_schema(&forecast, "ignored".to_string());
        assert_eq!(converted_forecast.area_uuid, "area_uuid");
        assert_eq!(converted_forecast.community_uuid, "comm_uuid");
        assert_eq!(converted_forecast.energy_kwh, 11.);
        assert_eq!(converted_forecast.confidence, 0.4);
        assert_eq!(converted_forecast.time_slot, 123123);
        assert_eq!(converted_forecast.creation_time, 456456);
    }

    #[test]
    fn test_convert_measurement_to_internal_schema() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let measurement = ExternalMeasurement {
            time_slot: 123123,
            creation_time: 456456,
            community_uuid: "comm_uuid".to_string(),
            energy_kwh: 11.,
            area_uuid: "area_uuid".to_string(),
        };
        let converted_measurement =
            adapter.convert_measurement_to_internal_schema(&measurement, "ignored".to_string());
        assert_eq!(converted_measurement.area_uuid, "area_uuid");
        assert_eq!(converted_measurement.community_uuid, "comm_uuid");
        assert_eq!(converted_measurement.energy_kwh, 11.);
        assert_eq!(converted_measurement.time_slot, 123123);
        assert_eq!(converted_measurement.creation_time, 456456);
    }
}
