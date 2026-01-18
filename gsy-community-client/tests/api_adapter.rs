use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{LECCommunityAssetsResults, LECCommunityMembersResults};
use gsy_community_client::types::{ExternalForecast, ExternalMeasurement};
use gsy_offchain_primitives::utils::h256_to_string;

use serde_json;
use subxt::utils::H256;

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
        let area_hash = h256_to_string(H256::random());
        let converted_forecast =
            adapter.convert_forecast_to_internal_schema(&forecast, area_hash.clone());
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
        let area_hash = h256_to_string(H256::random());
        let converted_measurement =
            adapter.convert_measurement_to_internal_schema(&measurement, area_hash.clone());
        assert_eq!(converted_measurement.area_uuid, "area_uuid");
        assert_eq!(converted_measurement.community_uuid, "comm_uuid");
        assert_eq!(converted_measurement.energy_kwh, 11.);
        assert_eq!(converted_measurement.time_slot, 123123);
        assert_eq!(converted_measurement.creation_time, 456456);
    }

    #[tokio::test]
    async fn test_import_external_topology() {
        let external_topology: String = r#"
    {
    "head": {
        "vars": [
            "lecName",
            "lecAltName",
            "siteName",
            "participantName"
        ]
    },
    "results": {
        "bindings": [
            {
                "lecName": {
                    "type": "literal",
                    "value": "Pilot1"
                },
                "lecAltName": {
                    "type": "literal",
                    "value": "Virtual Green H2 Federation"
                },
                "siteName": {
                    "type": "literal",
                    "value": "UrBeroaCommunity"
                },
                "participantName": {
                    "type": "literal",
                    "value": "UrBeroaMainStation"
                }
            },
            {
                "lecName": {
                    "type": "literal",
                    "value": "Pilot1"
                },
                "lecAltName": {
                    "type": "literal",
                    "value": "Virtual Green H2 Federation"
                },
                "siteName": {
                    "type": "literal",
                    "value": "UrBeroaCommunity"
                },
                "participantName": {
                    "type": "literal",
                    "value": "UrBeroaSubstation1"
                }
            }
        ]
    }
    }
    "#
        .to_string();

        let topology =
            serde_json::from_str::<LECCommunityMembersResults>(external_topology.as_str()).unwrap();
        assert_eq!(topology.results.bindings.len(), 2);
        assert_eq!(
            topology.results.bindings[0].site_name.value,
            "UrBeroaCommunity".to_string()
        );
        assert_eq!(
            topology.results.bindings[0].lec_name.value,
            "Pilot1".to_string()
        );
        assert_eq!(
            topology.results.bindings[0].participant_name.value,
            "UrBeroaMainStation".to_string()
        );
        assert_eq!(
            topology.results.bindings[1].site_name.value,
            "UrBeroaCommunity".to_string()
        );
        assert_eq!(
            topology.results.bindings[1].lec_name.value,
            "Pilot1".to_string()
        );
        assert_eq!(
            topology.results.bindings[1].participant_name.value,
            "UrBeroaSubstation1".to_string()
        );
    }

    #[tokio::test]
    async fn test_import_external_lec_assets() {
        let external_assets: String = r#"
    {
    "head": {
        "vars": [
            "location",
            "assetName",
            "assetType",
            "assetSubType"
        ]
    },
    "results": {
        "bindings": [
            {
                "location": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/characterization-main#LugaggiaInnovationCommunity"
                },
                "assetName": {
                    "type": "literal",
                    "value": "LIC02DBATT"
                },
                "assetType": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/battery#Battery"
                }
            },
            {
                "location": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/characterization-main#LugaggiaInnovationCommunity"
                },
                "assetName": {
                    "type": "literal",
                    "value": "LIC00SGIM"
                },
                "assetType": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/energyasset#Meter"
                },
                "assetSubType": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/energyasset#GridMeter"
                }
            },
            {
                "location": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/characterization-main#LugaggiaInnovationCommunity"
                },
                "assetName": {
                    "type": "literal",
                    "value": "LIC02SM"
                },
                "assetType": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/energyasset#Meter"
                },
                "assetSubType": {
                    "type": "uri",
                    "value": "http://w3id.org/fedecom/energyasset#SmartMeter"
                }
            }
        ]
    }
    }
    "#.to_string();

        let topology =
            serde_json::from_str::<LECCommunityAssetsResults>(external_assets.as_str()).unwrap();
        assert_eq!(topology.results.bindings.len(), 3);
        assert_eq!(
            topology.results.bindings[0].asset_name.value,
            "LIC02DBATT".to_string()
        );
        assert_eq!(
            topology.results.bindings[0].asset_type.value,
            "http://w3id.org/fedecom/battery#Battery".to_string()
        );
        assert_eq!(topology.results.bindings[0].asset_sub_type.is_none(), true);

        assert_eq!(
            topology.results.bindings[1].asset_name.value,
            "LIC00SGIM".to_string()
        );
        assert_eq!(
            topology.results.bindings[1].asset_type.value,
            "http://w3id.org/fedecom/energyasset#Meter".to_string()
        );
        assert_eq!(topology.results.bindings[1].asset_sub_type.is_some(), true);
        assert_eq!(
            topology.results.bindings[1]
                .asset_sub_type
                .clone()
                .unwrap()
                .value,
            "http://w3id.org/fedecom/energyasset#GridMeter"
        );

        assert_eq!(
            topology.results.bindings[2].asset_name.value,
            "LIC02SM".to_string()
        );
        assert_eq!(
            topology.results.bindings[2].asset_type.value,
            "http://w3id.org/fedecom/energyasset#Meter".to_string()
        );
        assert_eq!(topology.results.bindings[2].asset_sub_type.is_some(), true);
        assert_eq!(
            topology.results.bindings[2]
                .asset_sub_type
                .clone()
                .unwrap()
                .value,
            "http://w3id.org/fedecom/energyasset#SmartMeter"
        );
    }
}
