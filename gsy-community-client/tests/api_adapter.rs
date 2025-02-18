use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::external_api::{ExternalForecast, ExternalMeasurement};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;


#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;
    use tracing_subscriber;

    fn setup_tracing() {
        tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    }

    #[test]
    fn test_convert_forecast_to_internal_schema() {
        let adapter = AreaMarketInfoAdapter::new();
        let forecast = ExternalForecast {
            time_slot: 123123,
            creation_time: 456456,
            community_uuid: "comm_uuid",
            energy_kwh: 11.,
            area_uuid: "area_uuid"
        };
        adapter.convert_forecast_to_internal_schema()
        
        
    }
}