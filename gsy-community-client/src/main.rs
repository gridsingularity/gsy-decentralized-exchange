use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::manager::DemandForecastsManager;
use gsy_community_client::external_measurements::manager::MeasurementsManager;
use gsy_community_client::node_connector::orders::{publish_orders, calculate_order_rate};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use gsy_community_client::topology::TopologyManager;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use reqwest::Client;
use std::time::Duration;
use subxt_signer::sr25519::dev;
use tokio::time::sleep;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    measurements: MeasurementsManager,
    demand_forecasts: DemandForecastsManager,
    gsy_node_url: String,
}

impl AppState {
    fn new() -> Self {
        let api_adapter = AreaMarketInfoAdapter::new(None);
        AppState {
            client: Client::new(),
            api_adapter,
            measurements: MeasurementsManager::new(),
            demand_forecasts: DemandForecastsManager::new(),
            gsy_node_url: "http://gsy-node:9944/".to_string(),
        }
    }

    async fn poll_and_forward(&self) {
        loop {
            let seconds_since_epoch = get_current_timestamp_in_secs();

            let (_last_timeslot, next_timeslot) = get_last_and_next_timeslot();

            let internal_topology = TopologyManager::new(&self.client, &self.api_adapter)
                .get(next_timeslot)
                .await;

            self.measurements
                .fetch_and_forward(internal_topology.clone(), seconds_since_epoch)
                .await;

            // Gather the tradeable forecasts for every market once per slot. Only the
            // order price changes between resubmissions within the slot, so the energy
            // forecasts are fetched a single time here.
            let mut market_forecasts: Vec<(MarketTopologySchema, Vec<ForecastSchema>)> = Vec::new();
            for market in internal_topology.clone() {
                let valid_forecasts: Vec<ForecastSchema> = self
                    .demand_forecasts
                    .fetch_community_forecasts(&market, next_timeslot)
                    .await
                    .into_iter()
                    .filter(|forecast| {
                        self.api_adapter
                            .validate_forecast(forecast, seconds_since_epoch)
                    })
                    .collect();

                if valid_forecasts.is_empty() {
                    info!(
                        "No valid demand forecasts to forward for community {}.",
                        market.community_name
                    );
                    continue;
                }

                if let Err(e) = self
                    .api_adapter
                    .forward_forecast(valid_forecasts.clone())
                    .await
                {
                    info!("Failed to forward forecasts: {}", e);
                }

                // The API returns a multi-day forecast series, but only the points for the
                // upcoming timeslot are tradeable in this market.
                let next_timeslot_forecasts: Vec<ForecastSchema> = valid_forecasts
                    .into_iter()
                    .filter(|forecast| forecast.time_slot == market.time_slot as u64)
                    .collect();
                if next_timeslot_forecasts.is_empty() {
                    continue;
                }
                market_forecasts.push((market, next_timeslot_forecasts));
            }

            // Resubmit the orders several times within the slot, fluctuating only the
            // price each time. The number of resubmissions is derived from the slot
            // length and the configurable resubmission interval.
            let interval_sec = CommunityClientConstants.ORDER_RESUBMISSION_INTERVAL_SEC.max(1);
            let resubmissions = (GlobalConstants.TIME_SLOT_SEC / interval_sec).max(1);

            for _ in 0..resubmissions {
                let energy_rate = calculate_order_rate(
                    CommunityClientConstants.MIN_ORDER_RATE,
                    CommunityClientConstants.MAX_ORDER_RATE,
                );

                for (market, forecasts) in &market_forecasts {
                    if let Err(e) = publish_orders(
                        self.gsy_node_url.clone(),
                        forecasts.clone(),
                        market.clone(),
                        energy_rate,
                        &dev::alice(),
                    )
                    .await
                    {
                        error!(
                            "Failed to publish orders for community {}: {}",
                            market.community_name, e
                        );
                    }
                }

                // Spreading the resubmissions across the interval keeps the outer loop
                // roughly aligned with the market slot boundaries.
                sleep(Duration::from_secs(interval_sec)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let app_state = AppState::new();
    app_state.poll_and_forward().await;
}
