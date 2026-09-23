use gsy_community_client::offchain_storage_connector::adapter::{
    AreaMarketInfoAdapter, FORWARD_CHUNK_SIZE, build_new_market_topology, deterministic_area_hash,
    deterministic_area_uuid, deterministic_community_uuid, plan_residual_replacement,
};
use gsy_community_client::topology::{
    ExternalAreaTopology, ExternalCommunityTopology, LECCommunityAssetsResults,
    LECCommunityMembersResults, TopologyManager,
};
use gsy_community_client::types::{ExternalForecast, ExternalMeasurement};
use gsy_offchain_primitives::db_api_schema::market::AssetType;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOrderComponent, DbOrderSchema, Order, OrderStatus,
};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::h256_to_string;
use reqwest::Client;
use serde_json;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use subxt::utils::H256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[cfg(test)]
mod tests {
    use super::*;
    use gsy_community_client::time_utils::TIMESLOT_MINUTES;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn forecast_with(energy_kwh: f64, time_slot: u64) -> ForecastSchema {
        ForecastSchema {
            area_uuid: "area_uuid".to_string(),
            area_hash: h256_to_string(H256::random()),
            community_uuid: "comm_uuid".to_string(),
            time_slot,
            creation_time: 0,
            energy_kwh,
            confidence: 0.9,
        }
    }

    #[test]
    fn test_validate_forecast_accepts_negative_energy_production_offer() {
        // Negative energy = PV/production Offer; must pass with a future time slot.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        let forecast = forecast_with(-2.5, now + 900);
        assert!(adapter.validate_forecast(&forecast, now));
    }

    #[test]
    fn test_validate_forecast_rejects_zero_energy() {
        // Zero energy would produce no order, so it is dropped regardless of time slot.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        let forecast = forecast_with(0.0, now + 900);
        assert!(!adapter.validate_forecast(&forecast, now));
    }

    #[test]
    fn test_validate_forecast_accepts_positive_energy_consumption_bid() {
        // Regression: positive energy = demand Bid; still passes with a future time slot.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        let forecast = forecast_with(3.0, now + 900);
        assert!(adapter.validate_forecast(&forecast, now));
    }

    #[test]
    fn test_validate_forecast_rejects_past_time_slot_for_both_signs() {
        // A non-future time slot fails regardless of energy sign.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        // Strictly in the past.
        assert!(!adapter.validate_forecast(&forecast_with(3.0, now - 900), now));
        assert!(!adapter.validate_forecast(&forecast_with(-3.0, now - 900), now));
        // Equal to now is not strictly future, so it also fails.
        assert!(!adapter.validate_forecast(&forecast_with(3.0, now), now));
        assert!(!adapter.validate_forecast(&forecast_with(-3.0, now), now));
    }

    fn measurement_with(energy_kwh: f64, time_slot: u64) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: "area_uuid".to_string(),
            area_hash: h256_to_string(H256::random()),
            community_uuid: "comm_uuid".to_string(),
            time_slot,
            creation_time: 0,
            energy_kwh,
        }
    }

    #[test]
    fn test_validate_measurement_accepts_negative_energy_net_exporting_pv_meter() {
        // Regression for the bug: `energy_kwh` is the meter's signed net flow, so a
        // net-exporting PV meter (production) nets negative and must still be accepted.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        let measurement = measurement_with(-4.0, now - 900);
        assert!(adapter.validate_measurement(&measurement, now));
    }

    #[test]
    fn test_validate_measurement_accepts_positive_energy_net_importing_meter() {
        // No regression: a net-consuming meter still validates.
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        let measurement = measurement_with(4.0, now - 900);
        assert!(adapter.validate_measurement(&measurement, now));
    }

    #[test]
    fn test_validate_measurement_rejects_future_time_slot_for_both_signs() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        assert!(!adapter.validate_measurement(&measurement_with(3.0, now + 900), now));
        assert!(!adapter.validate_measurement(&measurement_with(-3.0, now + 900), now));
    }

    #[test]
    fn test_validate_measurement_rejects_non_finite_energy() {
        let adapter = AreaMarketInfoAdapter::new(None);
        let now = 1_000;
        assert!(!adapter.validate_measurement(&measurement_with(f64::NAN, now - 900), now));
        assert!(!adapter.validate_measurement(&measurement_with(f64::INFINITY, now - 900), now));
        assert!(!adapter.validate_measurement(
            &measurement_with(f64::NEG_INFINITY, now - 900),
            now
        ));
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

    #[test]
    fn test_asset_type_mapping_from_ontology_json() {
        let external_assets: String = r#"
        {
        "head": {
            "vars": ["location", "assetName", "assetType", "assetSubType"]
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

        let assets_result =
            serde_json::from_str::<LECCommunityAssetsResults>(external_assets.as_str()).unwrap();

        let manager = TopologyManager::new(&Client::new(), &AreaMarketInfoAdapter::new(None));
        let mapped = manager.map_assets_to_topology(assets_result);

        assert_eq!(mapped.len(), 3);

        // LIC02DBATT has assetType Battery → must classify as BATTERY, not AREA.
        let batt = mapped.iter().find(|a| a.area_name == "LIC02DBATT").unwrap();
        assert_eq!(
            batt.area_type,
            gsy_offchain_primitives::db_api_schema::market::AssetType::BATTERY,
            "LIC02DBATT should map to BATTERY"
        );

        // LIC00SGIM has assetType Meter + assetSubType GridMeter → must classify as GRID_METER.
        let grid = mapped.iter().find(|a| a.area_name == "LIC00SGIM").unwrap();
        assert_eq!(
            grid.area_type,
            gsy_offchain_primitives::db_api_schema::market::AssetType::GRID_METER,
            "LIC00SGIM should map to GRID_METER"
        );

        // LIC02SM has assetType Meter + assetSubType SmartMeter → must classify as SMART_METER.
        let smart = mapped.iter().find(|a| a.area_name == "LIC02SM").unwrap();
        assert_eq!(
            smart.area_type,
            gsy_offchain_primitives::db_api_schema::market::AssetType::SMART_METER,
            "LIC02SM should map to SMART_METER"
        );
    }

    #[tokio::test]
    async fn test_fetch_topology_returns_all_pilot_sites() {
        let manager = TopologyManager::new(&Client::new(), &AreaMarketInfoAdapter::new(None));
        const TIMESLOT_SECS: u64 = (TIMESLOT_MINUTES * 60) as u64;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let secs_since_last_timeslot = now % ((TIMESLOT_MINUTES * 60) as u64);
        let last_quarter = now - secs_since_last_timeslot;
        let pilot_sites = manager.fetch_topology().await.unwrap();
        let results = pilot_sites.results.bindings;
        let sites_names: HashSet<String> =
            results.iter().map(|x| x.site_name.value.clone()).collect();
        let lec_names: HashSet<String> = results.iter().map(|x| x.lec_name.value.clone()).collect();

        assert_eq!(
            sites_names,
            HashSet::from([
                "EZ_Puertollano".to_string(),
                "ArenaInnovationCommunity".to_string(),
                "EZ_Barcelona_TMB".to_string(),
                "UrBeroaCommunity".to_string(),
                "TownHall".to_string(),
                "GaramèDistrict".to_string(),
                "LugaggiaInnovationCommunity".to_string(),
                "ENBRO_Community".to_string(),
                "Brico_HQ".to_string()
            ])
        );
        assert_eq!(
            lec_names,
            HashSet::from([
                "Pilot1".to_string(),
                "Pilot2".to_string(),
                "Pilot3".to_string()
            ])
        );
    }

    fn test_topology() -> Vec<ExternalCommunityTopology> {
        vec![ExternalCommunityTopology {
            community_name: "TestCommunity".to_string(),
            areas: vec![ExternalAreaTopology {
                area_name: "TestArea".to_string(),
                area_type: AssetType::SMART_METER,
            }],
        }]
    }

    #[test]
    fn test_build_new_market_topology_area_hash_matches_across_timeslots() {
        let community = test_topology().into_iter().next().unwrap();
        let slot_a = build_new_market_topology(&community, 1_800_000_000);
        let slot_b = build_new_market_topology(&community, 1_800_000_900);

        assert_eq!(
            slot_a.community_uuid, slot_b.community_uuid,
            "community_uuid must be deterministic across timeslots"
        );
        assert_eq!(
            slot_a.community_areas[0].area_hash, slot_b.community_areas[0].area_hash,
            "area_hash must be deterministic across timeslots so a stored forecast matches the market"
        );
        assert_eq!(
            slot_a.community_uuid,
            deterministic_community_uuid("TestCommunity")
        );
        assert_eq!(
            slot_a.community_areas[0].area_hash,
            h256_to_string(deterministic_area_hash("TestCommunity", "TestArea"))
        );
    }

    #[test]
    fn test_build_new_market_topology_is_stable_on_repeat_calls() {
        let community = test_topology().into_iter().next().unwrap();
        let first = build_new_market_topology(&community, 1_800_001_800);
        let second = build_new_market_topology(&community, 1_800_001_800);
        assert_eq!(first, second);
    }

    #[test]
    fn test_stored_forecast_area_hash_matches_market_for_residual_replacement() {
        let time_slot = 1_800_003_600u64;
        let community = test_topology().into_iter().next().unwrap();
        let market = build_new_market_topology(&community, time_slot);
        let area = market.community_areas[0].clone();

        // The ingestion loop derives the same identity independently (it never sees the
        // `MarketTopologySchema` the publish loop built), so the stored forecast's
        // area_hash equals the market area's.
        let forecast = ForecastSchema {
            area_uuid: deterministic_area_uuid("TestCommunity", "TestArea"),
            area_hash: h256_to_string(deterministic_area_hash("TestCommunity", "TestArea")),
            community_uuid: deterministic_community_uuid("TestCommunity"),
            time_slot,
            creation_time: time_slot - 3_600,
            energy_kwh: 3.0,
            confidence: 0.9,
        };
        assert_eq!(forecast.area_hash, area.area_hash);
        let stored = vec![forecast.clone()];

        // `create_input_orders` (orders.rs) stamps `OrderComponent.area_uuid` from the
        // forecast's `area_hash`, so an open bid for this area carries `area_uuid ==
        // forecast.area_hash`; `plan_residual_replacement` must find and replace it.
        let trader = "trader".to_string();
        let open_bid = DbOrderSchema {
            _id: format!("0x{}", "11".repeat(32)),
            status: OrderStatus::Open,
            order: Order::Bid(DbBid {
                buyer: trader.clone(),
                nonce: 0,
                bid_component: DbOrderComponent {
                    area_uuid: stored[0].area_hash.clone(),
                    market_id: market.market_id.clone(),
                    time_slot,
                    creation_time: forecast.creation_time,
                    energy: 2.0,
                    energy_rate: 0.1,
                },
            }),
        };

        let (hashes_to_delete, adjusted) = plan_residual_replacement(&[open_bid], &trader, stored);
        assert_eq!(
            hashes_to_delete.len(),
            1,
            "the matching open order must be scheduled for deletion"
        );
        assert_eq!(adjusted.len(), 1);
        assert_eq!(
            adjusted[0].energy_kwh, 2.0,
            "residual energy from the open order replaces the raw forecast"
        );
    }

    /// A one-shot HTTP/1.1 stub the adapter can be pointed at. There is no HTTP mocking
    /// crate in this workspace, so the forwarding tests bring their own minimal server: it
    /// answers every request with the same, configurable status line and records the raw
    /// request bodies it received, which is all the assertions below need.
    struct StubServer {
        /// Base URL to hand to [`AreaMarketInfoAdapter::new`].
        url: String,
        bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl StubServer {
        fn bodies(&self) -> Vec<Vec<u8>> {
            self.bodies.lock().unwrap().clone()
        }

        fn request_count(&self) -> usize {
            self.bodies.lock().unwrap().len()
        }
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    /// Read one full HTTP request off `stream` and return its body. The bodies here are
    /// hundreds of kilobytes, so the headers are parsed for `Content-Length` and the body
    /// is then read in a loop until exactly that many bytes have arrived.
    async fn read_request_body(stream: &mut TcpStream) -> Option<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];

        let header_end = loop {
            if let Some(position) = find_subsequence(&buffer, b"\r\n\r\n") {
                break position + 4;
            }
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            buffer.extend_from_slice(&chunk[..read]);
        };

        let headers = String::from_utf8_lossy(&buffer[..header_end]).to_ascii_lowercase();
        // Defensive: reqwest does not use it today, but an `Expect: 100-continue` would
        // otherwise deadlock the read below, since the client waits for the interim reply.
        if headers.contains("expect: 100-continue") {
            stream
                .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
                .await
                .ok()?;
        }
        let content_length: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);

        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }
        Some(buffer[header_end..header_end + content_length].to_vec())
    }

    /// Start the stub on an ephemeral loopback port. `status_line` is the full HTTP status
    /// line to answer with, e.g. `"HTTP/1.1 413 Payload Too Large"`.
    async fn start_stub_server(status_line: &'static str) -> StubServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let bodies: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&bodies);

        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                if let Some(body) = read_request_body(&mut stream).await {
                    recorded.lock().unwrap().push(body);
                }
                let response =
                    format!("{status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                let _ = stream.shutdown().await;
            }
        });

        StubServer {
            url: format!("http://{address}"),
            bodies,
        }
    }

    #[tokio::test]
    async fn test_forward_forecast_splits_large_payload_into_chunks() {
        // A real day-ahead ingest is thousands of points in one body, which the storage
        // service rejects with 413; the adapter must split it into bounded requests.
        let total = 2 * FORWARD_CHUNK_SIZE + 1;
        let server = start_stub_server("HTTP/1.1 200 OK").await;
        let adapter = AreaMarketInfoAdapter::new(Some(server.url.clone()));
        let forecasts: Vec<ForecastSchema> = (0..total)
            .map(|index| forecast_with(1.5, 1_800_000_000 + index as u64))
            .collect();

        adapter
            .forward_forecast(forecasts)
            .await
            .expect("forwarding against a 200 server must succeed");

        let bodies = server.bodies();
        assert_eq!(bodies.len(), 3, "2001 points must be sent as 3 requests");
        let mut forwarded = 0usize;
        for body in &bodies {
            let chunk: Vec<ForecastSchema> =
                serde_json::from_slice(body).expect("each body is a JSON array of forecasts");
            assert!(
                chunk.len() <= FORWARD_CHUNK_SIZE,
                "no request may exceed FORWARD_CHUNK_SIZE points, got {}",
                chunk.len()
            );
            forwarded += chunk.len();
        }
        assert_eq!(
            forwarded, total,
            "every point must be forwarded exactly once"
        );
    }

    #[tokio::test]
    async fn test_forward_forecast_returns_error_on_rejected_payload() {
        // Regression: the status used to be ignored, so a 413 was logged as a successful
        // ingest and the forecasts were silently lost.
        let server = start_stub_server("HTTP/1.1 413 Payload Too Large").await;
        let adapter = AreaMarketInfoAdapter::new(Some(server.url.clone()));
        let forecasts: Vec<ForecastSchema> = (0..(FORWARD_CHUNK_SIZE + 5))
            .map(|index| forecast_with(1.5, 1_800_000_000 + index as u64))
            .collect();

        let result = adapter.forward_forecast(forecasts).await;

        assert!(result.is_err(), "a 413 response must surface as an error");
        assert_eq!(
            server.request_count(),
            1,
            "forwarding must stop at the first failing chunk"
        );
    }

    #[tokio::test]
    async fn test_forward_forecast_sends_nothing_for_empty_input() {
        let server = start_stub_server("HTTP/1.1 200 OK").await;
        let adapter = AreaMarketInfoAdapter::new(Some(server.url.clone()));

        adapter
            .forward_forecast(vec![])
            .await
            .expect("an empty forecast list is a no-op, not a failure");

        assert_eq!(
            server.request_count(),
            0,
            "an empty forecast list must not hit the network"
        );
    }

    #[tokio::test]
    async fn test_forward_measurement_returns_error_on_rejected_payload() {
        let server = start_stub_server("HTTP/1.1 413 Payload Too Large").await;
        let adapter = AreaMarketInfoAdapter::new(Some(server.url.clone()));
        let measurements: Vec<MeasurementSchema> = (0..10)
            .map(|index| measurement_with(1.5, 1_800_000_000 + index as u64))
            .collect();

        let result = adapter.forward_measurement(measurements).await;

        assert!(result.is_err(), "a 413 response must surface as an error");
        assert_eq!(server.request_count(), 1);
    }
}
