use crate::constants::INTER_COMMUNITY_MARKET_NAME;
use crate::inter_community::inter_community_market_id;
use crate::time_utils::get_current_timestamp_in_secs;
use crate::topology::ExternalCommunityTopology;
use crate::types::{ExternalForecast, ExternalMeasurement};
use blake2_rfc::blake2b::blake2b;
use gsy_offchain_primitives::MarketType;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::orders::{DbOrderSchema, Order, OrderStatus};
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::utils::{h256_to_string, string_to_h256};
use reqwest::Client;
use subxt::utils::H256;
use tracing::{error, info};
use uuid::Uuid;

const RESIDUAL_ENERGY_TOLERANCE_KWH: f64 = 1e-9;

pub fn generate_market_id(
    community_name: &str,
    market_type: MarketType,
    delivery_timestamp: u64,
) -> H256 {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(community_name.as_bytes());
    buffer.extend_from_slice(market_type.as_str().as_bytes());
    buffer.extend_from_slice(&delivery_timestamp.to_be_bytes());
    H256(
        blake2b(32, &[], &buffer)
            .as_bytes()
            .try_into()
            .expect("hash is 32 bytes"),
    )
}

#[derive(Clone, Debug)]
pub struct AreaMarketInfoAdapter {
    client: Client,
    internal_forecast_url: String,
    internal_measurements_url: String,
    internal_orders_url: String,
    pub internal_topology_url: String,
    pub internal_community_market_url: String,
}

impl AreaMarketInfoAdapter {
    pub fn new(host: Option<String>) -> Self {
        let hostname = host.unwrap_or_else(|| "http://gsy-orderbook:8080".to_string());
        AreaMarketInfoAdapter {
            client: Client::new(),
            internal_forecast_url: hostname.clone() + "/forecasts",
            internal_measurements_url: hostname.clone() + "/measurements",
            internal_orders_url: hostname.clone() + "/orders",
            internal_topology_url: hostname.clone() + "/market",
            internal_community_market_url: hostname.clone() + "/community-market",
        }
    }

    /// Fetch the orders for market_id from the off-chain storage.
    pub async fn get_orders_for_market(&self, market_id: &str) -> Vec<DbOrderSchema> {
        let url = format!("{}?market_id={}", self.internal_orders_url, market_id);
        let response = match self.client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                error!("Fetching market orders failed with status: {}", resp.status());
                return vec![];
            }
            Err(err) => {
                error!("Fetching market orders failed: {:?}", err);
                return vec![];
            }
        };
        response.json::<Vec<DbOrderSchema>>().await.unwrap_or_else(|err| {
            error!("Failed to deserialize market orders response: {:?}", err);
            vec![]
        })
    }

    // Function to forward the forecast data to internal API
    pub async fn forward_forecast(
        &self,
        forecasts: Vec<ForecastSchema>,
    ) -> Result<(), reqwest::Error> {
        self.client
            .post(&self.internal_forecast_url)
            .json(&forecasts)
            .send()
            .await?;
        Ok(())
    }

    // Function to forward the measurement data to internal API
    pub async fn forward_measurement(
        &self,
        measurements: Vec<MeasurementSchema>,
    ) -> Result<(), reqwest::Error> {
        self.client
            .post(&self.internal_measurements_url)
            .json(&measurements)
            .send()
            .await?;
        Ok(())
    }

    // Validation logic (basic validation, can be extended)
    pub fn validate_forecast(&self, forecast: &ForecastSchema, seconds_since_epoch: u64) -> bool {
        forecast.energy_kwh > 0.0 && forecast.time_slot > seconds_since_epoch
    }

    pub fn validate_measurement(
        &self,
        measurement: &MeasurementSchema,
        seconds_since_epoch: u64,
    ) -> bool {
        measurement.energy_kwh > 0.0 && measurement.time_slot <= seconds_since_epoch
    }

    pub fn convert_forecast_to_internal_schema(
        &self,
        forecast: &ExternalForecast,
        area_hash: String,
    ) -> ForecastSchema {
        ForecastSchema {
            area_uuid: forecast.area_uuid.clone(),
            area_hash: area_hash.clone(),
            community_uuid: forecast.community_uuid.clone(),
            time_slot: forecast.time_slot,
            creation_time: forecast.creation_time,
            energy_kwh: forecast.energy_kwh,
            confidence: forecast.confidence,
        }
    }

    pub fn convert_measurement_to_internal_schema(
        &self,
        measurement: &ExternalMeasurement,
        area_hash: String,
    ) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: measurement.area_uuid.clone(),
            area_hash: area_hash.clone(),
            community_uuid: measurement.community_uuid.clone(),
            time_slot: measurement.time_slot,
            creation_time: measurement.creation_time,
            energy_kwh: measurement.energy_kwh,
        }
    }

    pub async fn get_existing_market_topology(
        &self,
        community_market_url: String,
    ) -> Vec<MarketTopologySchema> {
        let response = match self.client.get(community_market_url).send().await {
            Ok(resp) if resp.status().is_success() => resp,
            _ => return vec![],
        };
        response
            .json::<Vec<MarketTopologySchema>>()
            .await
            .unwrap_or_else(|err| {
                error!("Failed to deserialize market topology response: {:?}", err);
                vec![]
            })
    }

    pub async fn get_or_create_market_topology(
        &self,
        topology: Vec<ExternalCommunityTopology>,
        time_slot: u64,
    ) -> Vec<MarketTopologySchema> {
        let mut market_topologies: Vec<MarketTopologySchema> = vec![];
        for community_topology in topology {
            let community_market_url = format!(
                "{}?community_name={}&start_time={}&end_time={}",
                self.internal_community_market_url,
                community_topology.community_name,
                time_slot,
                time_slot
            );
            let market_topology_res = self
                .get_existing_market_topology(community_market_url)
                .await;
            if !market_topology_res.is_empty() {
                market_topologies.push(market_topology_res.get(0).unwrap().clone());
            } else {
                let new_market = MarketTopologySchema {
                    community_name: community_topology.community_name.clone(),
                    community_uuid: Uuid::new_v4().to_string(),
                    market_id: h256_to_string(generate_market_id(
                        &community_topology.community_name,
                        MarketType::Spot,
                        time_slot,
                    )),
                    time_slot: time_slot as u32,
                    creation_time: get_current_timestamp_in_secs() as u32,
                    community_areas: community_topology
                        .areas
                        .clone()
                        .into_iter()
                        .map(|area| AreaTopologySchema {
                            area_uuid: Uuid::new_v4().to_string(),
                            area_type: area.area_type.clone(),
                            name: area.area_name.clone(),
                            area_hash: h256_to_string(H256::random()),
                        })
                        .collect(),
                };
                let topology_resp = self
                    .client
                    .post(&self.internal_topology_url)
                    .json(&new_market)
                    .send()
                    .await;

                match topology_resp {
                    Ok(_) => market_topologies.push(new_market.clone()),
                    Err(error) => {
                        info!(
                            "New topology creation failed with error: {}",
                            error.to_string()
                        );
                    }
                }
            }
        }
        market_topologies
    }

    /// Upsert the single inter-community market for a timeslot. Its id is
    /// community-independent, so this must be called once per timeslot, outside any
    /// per-community loop; storage-side dedup is keyed on `market_id`.
    pub async fn get_or_create_inter_community_market(
        &self,
        time_slot: u64,
    ) -> Option<MarketTopologySchema> {
        let community_market_url = format!(
            "{}?community_name={}&start_time={}&end_time={}",
            self.internal_community_market_url, INTER_COMMUNITY_MARKET_NAME, time_slot, time_slot
        );
        let existing = self
            .get_existing_market_topology(community_market_url)
            .await;
        if let Some(market) = existing.into_iter().next() {
            return Some(market);
        }
        let new_market = MarketTopologySchema {
            community_name: INTER_COMMUNITY_MARKET_NAME.to_string(),
            community_uuid: Uuid::new_v4().to_string(),
            market_id: h256_to_string(inter_community_market_id(time_slot)),
            time_slot: time_slot as u32,
            creation_time: get_current_timestamp_in_secs() as u32,
            community_areas: vec![],
        };
        match self
            .client
            .post(&self.internal_topology_url)
            .json(&new_market)
            .send()
            .await
        {
            Ok(_) => Some(new_market),
            Err(error) => {
                info!(
                    "Inter-community market creation failed with error: {}",
                    error.to_string()
                );
                None
            }
        }
    }
}

/// Parse an off-chain order `_id` (a `0x`-prefixed 32-byte hex string) into an `H256`.
fn parse_order_hash(id: &str) -> Option<H256> {
    let hex = id.strip_prefix("0x")?;
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(string_to_h256(id.to_string()))
}

/// Plan the replacement of `trader`'s open orders for a single market.
///
/// Given `open_orders` (everything currently stored for the market) and the `forecasts`
/// about to be published for it, returns:
/// - the on-chain hashes of `trader`'s open orders that should be deleted first, and
/// - the forecasts adjusted so each one carries over its area's residual (still-open)
///   energy instead of the raw forecast value.
///
/// For every forecast whose area already has open orders of the matching side (bid for
/// consumption, offer for generation), those orders are scheduled for deletion and the
/// forecast energy is replaced by their summed residual energy. A partially-traded order
/// is therefore replaced rather than re-inflated to the full forecast, and a fully-traded
/// one is dropped (no replacement order). Forecasts for areas without an existing open
/// order keep their forecast energy (first publication for that area).
pub fn plan_residual_replacement(
    open_orders: &[DbOrderSchema],
    trader: &str,
    forecasts: Vec<ForecastSchema>,
) -> (Vec<H256>, Vec<ForecastSchema>) {
    let mut hashes_to_delete: Vec<H256> = Vec::new();
    let mut adjusted_forecasts: Vec<ForecastSchema> = Vec::new();

    for mut forecast in forecasts {
        let is_bid = forecast.energy_kwh > 0.0;
        let mut residual_energy = 0.0;
        let mut matched_existing = false;

        for stored in open_orders {
            if stored.status != OrderStatus::Open {
                continue;
            }
            // Match the order to this forecast's area and side, and only the trader's own.
            let component = match (&stored.order, is_bid) {
                (Order::Bid(bid), true) if bid.buyer == trader => &bid.bid_component,
                (Order::Offer(offer), false) if offer.seller == trader => &offer.offer_component,
                _ => continue,
            };
            if component.area_uuid != forecast.area_hash {
                continue;
            }
            residual_energy += component.energy;
            matched_existing = true;
            if let Some(hash) = parse_order_hash(&stored._id) {
                hashes_to_delete.push(hash);
            }
        }

        if matched_existing {
            // Carry over the residual energy; skip publishing if nothing is left to trade.
            if residual_energy <= RESIDUAL_ENERGY_TOLERANCE_KWH {
                continue;
            }
            forecast.energy_kwh = if is_bid { residual_energy } else { -residual_energy };
        }
        adjusted_forecasts.push(forecast);
    }

    (hashes_to_delete, adjusted_forecasts)
}
