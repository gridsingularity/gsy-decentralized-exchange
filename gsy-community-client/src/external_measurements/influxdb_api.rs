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
    pub fn net_energy_Wh(&self) -> f64 {
        self.import_Wh - self.export_Wh
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
        let client = ReqwestClient::new();
        let response = client
            .post(self.url())
            .header("Accept", "application/json")
            .header("Authorization", "Token ".to_string() + self.token.as_str())
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .unwrap();
        let response_text = response.text().await.unwrap();

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
            let sensor_id_tokens = record.sensor_id.split('-');
            // TODO: For now only FLEXO sensors are integrated in InfluxDB.
            assert_eq!(
                sensor_id_tokens.clone().nth(0).unwrap().to_string(),
                "FLEXO".to_string()
            );
            let smart_meter_id = sensor_id_tokens.clone().nth(2).unwrap().to_string();
            let measurement_type = sensor_id_tokens.clone().nth(3).unwrap().to_string();
            if !smart_meter_measurements.contains_key(&smart_meter_id) {
                smart_meter_measurements.insert(smart_meter_id.clone(), HashMap::new());
                smart_meter_measurements
                    .get_mut(&smart_meter_id)
                    .unwrap()
                    .insert(
                        record.time,
                        InfluxMeasurementMeterData {
                            sensor_id: smart_meter_id.clone(),
                            time: record.time,
                            import_Wh: 0.,
                            export_Wh: 0.,
                            consumption_Wh: 0.,
                            export_pv_Wh: 0.,
                        },
                    );
            }

            // Get nested reference for smart meter id and timestamp
            let hashmap_reference = smart_meter_measurements
                .get_mut(&smart_meter_id)
                .unwrap()
                .get_mut(&record.time)
                .unwrap();
            match measurement_type.as_str() {
                "import" => hashmap_reference.import_Wh = record.value.unwrap(),
                "export" => hashmap_reference.export_Wh = record.value.unwrap(),
                "consumption" => hashmap_reference.consumption_Wh = record.value.unwrap(),
                "export_pv" => hashmap_reference.export_pv_Wh = record.value.unwrap(),
                _ => error!("Unknown measurement type: {}", measurement_type),
            }
        }
        smart_meter_measurements
    }
}
