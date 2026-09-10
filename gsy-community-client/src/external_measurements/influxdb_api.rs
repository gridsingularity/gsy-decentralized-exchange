#![allow(non_snake_case, non_upper_case_globals)]
use crate::constants::CommunityClientConstants;
use chrono::{DateTime, SecondsFormat, Utc};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

#[derive(Serialize, Deserialize, Debug)]
struct RawInfluxDBMeasurement {
    #[serde(rename = "result")]
    result: String,

    table: u32,

    #[serde(rename = "_start")]
    start: DateTime<Utc>,

    #[serde(rename = "_stop")]
    stop: DateTime<Utc>,

    #[serde(rename = "_time")]
    time: DateTime<Utc>,

    #[serde(rename = "_value")]
    value: Option<f64>,

    #[serde(rename = "_field")]
    field: String,

    #[serde(rename = "_measurement")]
    measurement: String,

    sensor_id: String,
    topic: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct InfluxRequestParams {
    query: String,
    org_id: String,
}

#[derive(Debug, Clone)]
pub struct InfluxMeasurementMeterData {
    pub sensor_id: String,
    pub time: DateTime<Utc>,
    pub import_Wh: f64,
    pub export_Wh: f64,
    pub consumption_Wh: f64,
    pub export_pv_Wh: f64,
}

impl InfluxMeasurementMeterData {
    pub fn net_energy_kWh(&self) -> f64 {
        (self.import_Wh - self.export_Wh) / 1000.0
    }

    pub fn export_pv_kWh(&self) -> f64 {
        self.export_pv_Wh / 1000.0
    }
}

#[derive(Clone)]
pub struct MeasurementInfluxDBConnection {
    address: String,
    org: String,
    token: String,
}

impl MeasurementInfluxDBConnection {
    pub fn new() -> Self {
        MeasurementInfluxDBConnection {
            address: CommunityClientConstants.FEDECOM_INFLUX_DB_URL.clone(),
            org: CommunityClientConstants.FEDECOM_INFLUX_DB_ORG.clone(),
            token: CommunityClientConstants.FEDECOM_INFLUX_DB_TOKEN.clone(),
        }
    }

    fn url(&self) -> String {
        format!("{}?org={}", self.address, self.org)
    }

    async fn fetch_from_db(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Vec<RawInfluxDBMeasurement> {
        let query_str = format!(
            r#"from(bucket: "fedecom")
          |> range(start: {}, stop: {})
          |> filter(fn: (r) => r["_measurement"] == "active_energy")
          |> filter(fn: (r) => r["sensor_id"] =~ /^FLEXO-.*/)
          |> filter(fn: (r) => not r["sensor_id"] =~ /^FLEXO-AIC-49.*/)
        "#,
            start_time
                .to_rfc3339_opts(SecondsFormat::Secs, true)
                .to_string(),
            end_time
                .to_rfc3339_opts(SecondsFormat::Secs, true)
                .to_string(),
        );

        let request_body = InfluxRequestParams {
            query: query_str.clone(),
            org_id: CommunityClientConstants.FEDECOM_INFLUX_DB_ORG.clone(),
        };
        let client = ReqwestClient::builder()
            .timeout(std::time::Duration::from_secs(
                CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
            ))
            .connect_timeout(std::time::Duration::from_secs(
                CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
            ))
            .build()
            .expect("Failed to build InfluxDB HTTP client");
        // Every failure here returns an empty result set rather than panicking. This runs at
        // the tail of the community client's publish loop, so a panic would take that task
        // down permanently while the process stays alive (the ingest task keeps the
        // container running), silently ending order publication until someone restarts it.
        let response = match client
            .post(self.url())
            .header("Accept", "application/json")
            .header("Authorization", "Token ".to_string() + self.token.as_str())
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to query InfluxDB at {}: {}", self.address, e);
                return Vec::new();
            }
        };
        if !response.status().is_success() {
            error!("InfluxDB query failed with status: {}", response.status());
            return Vec::new();
        }
        let response_text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                error!("Failed to read the InfluxDB response body: {}", e);
                return Vec::new();
            }
        };

        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(response_text.as_bytes());

        rdr.deserialize::<RawInfluxDBMeasurement>()
            .filter_map(|result| result.ok())
            .collect::<Vec<RawInfluxDBMeasurement>>()
    }

    pub async fn read(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> HashMap<String, HashMap<DateTime<Utc>, InfluxMeasurementMeterData>> {
        let mut smart_meter_measurements: HashMap<
            String,
            HashMap<DateTime<Utc>, InfluxMeasurementMeterData>,
        > = HashMap::new();
        let fetched_data = self.fetch_from_db(start_time, end_time).await;
        for record in fetched_data.iter() {
            // TODO: For now only FLEXO sensors are integrated in InfluxDB.
            // The Flux query already filters on the `FLEXO-` prefix, but an id that does not
            // split into `FLEXO-<x>-<meter>-<type>` is skipped rather than asserted on, for
            // the same reason `fetch_from_db` never panics.
            let sensor_id_tokens: Vec<&str> = record.sensor_id.split('-').collect();
            if sensor_id_tokens.len() < 4 || sensor_id_tokens[0] != "FLEXO" {
                error!(
                    "Skipping unrecognized InfluxDB sensor id: {}",
                    record.sensor_id
                );
                continue;
            }
            let smart_meter_id = sensor_id_tokens[2].to_string();
            let measurement_type = sensor_id_tokens[3];

            // A null `_value` is a gap in the series, not a zero reading; skip it so it
            // neither panics nor pulls the meter's net energy towards zero.
            let Some(value) = record.value else {
                continue;
            };

            let meter_data = smart_meter_measurements
                .entry(smart_meter_id.clone())
                .or_default()
                .entry(record.time)
                .or_insert_with(|| InfluxMeasurementMeterData {
                    sensor_id: smart_meter_id.clone(),
                    time: record.time,
                    import_Wh: 0.,
                    export_Wh: 0.,
                    consumption_Wh: 0.,
                    export_pv_Wh: 0.,
                });
            match measurement_type {
                "import" => meter_data.import_Wh = value,
                "export" => meter_data.export_Wh = value,
                "consumption" => meter_data.consumption_Wh = value,
                "export_pv" => meter_data.export_pv_Wh = value,
                _ => error!("Unknown measurement type: {}", measurement_type),
            }
        }
        smart_meter_measurements
    }
}

