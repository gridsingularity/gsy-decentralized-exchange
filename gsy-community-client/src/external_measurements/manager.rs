use crate::external_measurements::influxdb_api::MeasurementInfluxDBConnection;
use crate::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use crate::types::ExternalMeasurement;
use chrono::Utc;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::MeasurementSchema;
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::time::Duration;
use tracing::info;

#[derive(Clone)]
pub struct MeasurementsManager {
    external_measurements_api: MeasurementInfluxDBConnection,
    offchain_storage_api: AreaMarketInfoAdapter,
}

impl MeasurementsManager {
    pub fn new() -> Self {
        MeasurementsManager {
            external_measurements_api: MeasurementInfluxDBConnection::new(),
            offchain_storage_api: AreaMarketInfoAdapter::new(None),
        }
    }

    // Function to fetch an array of measurement data
    async fn fetch_measurements(
        &self,
        topologies: Vec<MarketTopologySchema>,
    ) -> Vec<ExternalMeasurement> {
        let start_time = Utc::now() - Duration::from_secs(2 * GlobalConstants.TIME_SLOT_SEC);
        let end_time = Utc::now();
        let measurements = self
            .external_measurements_api
            .read(start_time, end_time)
            .await;

        let mut external_measurements: Vec<ExternalMeasurement> = vec![];
        for topology in topologies.iter() {
            let topology_member_ids: HashSet<String> = HashSet::from_iter(
                topology
                    .community_areas
                    .iter()
                    .map(|area| area.name.clone()),
            );
            for (sensor_id, timestamp_hashmap) in measurements.clone().into_iter() {
                // TODO: Create a manual mapping between ontology sensor ids and Influx sensor ids
                if topology_member_ids.contains(&sensor_id) {
                    // This sensor is part of the community. Create external measurements.
                    for (timestamp, record) in timestamp_hashmap.clone().into_iter() {
                        external_measurements.push(ExternalMeasurement {
                            community_uuid: topology.community_uuid.clone(),
                            area_uuid: sensor_id.clone(),
                            time_slot: timestamp.timestamp() as u64,
                            creation_time: Utc::now().timestamp() as u64,
                            energy_kwh: record.net_energy_Wh(),
                        })
                    }
                }
            }
        }
        external_measurements
    }

    pub async fn fetch_and_forward(
        &self,
        internal_topology: Vec<MarketTopologySchema>,
        seconds_since_epoch: u64,
    ) {
        let area_uuid_to_hash: HashMap<String, String> = internal_topology
            .iter()
            .flat_map(|topology| topology.community_areas.iter())
            .map(|area| (area.area_uuid.clone(), area.area_hash.clone()))
            .collect();

        // Fetch and forward measurements
        let measurements = self.fetch_measurements(internal_topology.clone()).await;
        let valid_measurements: Vec<MeasurementSchema> = measurements
            .into_iter()
            .map(|measurement| {
                self.offchain_storage_api
                    .convert_measurement_to_internal_schema(
                        &measurement,
                        area_uuid_to_hash[&measurement.area_uuid].clone(),
                    )
            })
            .filter(|measurement| {
                self.offchain_storage_api
                    .validate_measurement(measurement, seconds_since_epoch)
            })
            .collect();
        if !valid_measurements.is_empty() {
            if let Err(e) = self
                .offchain_storage_api
                .forward_measurement(valid_measurements)
                .await
            {
                info!("Failed to forward measurements: {}", e);
            }
        } else {
            info!("No valid measurements to forward.");
        }
    }
}
