use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::manager::DemandForecastsManager;
use gsy_community_client::external_measurements::manager::MeasurementsManager;
use gsy_community_client::inter_community::eligible_inter_community;
use gsy_community_client::node_connector::orders::{
    calculate_order_rate, create_inter_community_order, publish_input_orders, publish_orders,
    remove_orders,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    AreaMarketInfoAdapter, plan_residual_replacement,
};
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, open_spot_market_timeslots};
use gsy_community_client::topology::TopologyManager;
use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{community_id_from_uuid, h256_to_string, string_to_h256};
use reqwest::Client;
use std::collections::HashSet;
use std::time::Duration;
use subxt::utils::AccountId32;
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
            client: Client::builder()
                .timeout(Duration::from_secs(
                    CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
                ))
                .connect_timeout(Duration::from_secs(
                    CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
                ))
                .build()
                .expect("Failed to build topology HTTP client"),
            api_adapter,
            measurements: MeasurementsManager::new(),
            demand_forecasts: DemandForecastsManager::new(),
            gsy_node_url: "http://gsy-node:9944/".to_string(),
        }
    }

    /// Publish at most one aggregated net order per eligible community into the
    /// inter-community market, replacing (not stacking) the community's previous order.
    async fn publish_inter_community_orders(
        &self,
        inter_market: &MarketTopologySchema,
        timeslot: u64,
        now: u64,
        bid_rate: f64,
        offer_rate: f64,
        trader: &str,
        community_forecasts: Vec<(String, String, Vec<ForecastSchema>)>,
    ) {
        let market_id = string_to_h256(inter_market.market_id.clone());
        for (community_name, community_uuid, forecasts) in community_forecasts {
            // TODO: include production/PV forecasts in the net once that source lands
            // (handled in a separate effort); the demand forecaster currently supplies
            // consumption only, so the net degenerates to pure consumption.
            let net_import_kwh = aggregate_net_import(&forecasts, &community_uuid, timeslot);
            let community_id = community_id_from_uuid(&community_uuid);

            // Residual replacement keyed on community_id so a re-tick replaces the
            // community's single order rather than stacking a new one.
            let net_forecast = ForecastSchema {
                area_uuid: community_uuid.clone(),
                area_hash: h256_to_string(community_id),
                community_uuid: community_uuid.clone(),
                time_slot: timeslot,
                creation_time: now,
                energy_kwh: net_import_kwh,
                confidence: 1.0,
            };
            let open_orders = self
                .api_adapter
                .get_orders_for_market(&inter_market.market_id)
                .await;
            let (hashes_to_delete, adjusted) =
                plan_residual_replacement(&open_orders, trader, vec![net_forecast]);

            if let Err(e) =
                remove_orders(self.gsy_node_url.clone(), hashes_to_delete, &dev::alice()).await
            {
                error!(
                    "Failed to remove previous inter-community order for community {}: {}",
                    community_name, e
                );
            }

            let Some(replacement) = adjusted.into_iter().next() else {
                continue;
            };
            let rate = if replacement.energy_kwh > 0.0 {
                bid_rate
            } else {
                offer_rate
            };
            let Some(order) = create_inter_community_order(
                replacement.energy_kwh,
                community_id,
                market_id,
                timeslot,
                rate,
                &dev::alice(),
            ) else {
                continue;
            };

            if let Err(e) =
                publish_input_orders(self.gsy_node_url.clone(), vec![order], &dev::alice()).await
            {
                error!(
                    "Failed to publish inter-community order for community {}: {}",
                    community_name, e
                );
            }
        }
    }

    async fn poll_and_forward(&self) {
        // The client re-posts orders to every open market on this interval.
        let interval_sec = CommunityClientConstants.ORDER_RESUBMISSION_INTERVAL_SEC.max(1);

        // The account every order is signed with.
        let trader = AccountId32::from(dev::alice().public_key()).to_string();

        loop {
            let now = get_current_timestamp_in_secs();

            let open_timeslots = open_spot_market_timeslots(now);
            if open_timeslots.is_empty() {
                info!("No spot markets are currently open for order submission.");
                sleep(Duration::from_secs(interval_sec)).await;
                continue;
            }

            let markets_per_timeslot = TopologyManager::new(&self.client, &self.api_adapter)
                .get_for_timeslots(&open_timeslots)
                .await;

            let mut measurement_topologies: Vec<MarketTopologySchema> = Vec::new();
            let mut seen_communities: HashSet<String> = HashSet::new();

            for (timeslot, markets) in markets_per_timeslot {
                let (open_time, close_time) = GlobalConstants.spot_market_window(timeslot);
                let bid_rate = calculate_order_rate(
                    CommunityClientConstants.MIN_ORDER_RATE,
                    CommunityClientConstants.MAX_ORDER_RATE,
                    now,
                    open_time,
                    close_time,
                    true,
                );
                let offer_rate = calculate_order_rate(
                    CommunityClientConstants.MIN_ORDER_RATE,
                    CommunityClientConstants.MAX_ORDER_RATE,
                    now,
                    open_time,
                    close_time,
                    false,
                );

                // Single inter-community market per timeslot, created outside the
                // per-community loop (its id is community-independent).
                let inter_market = self
                    .api_adapter
                    .get_or_create_inter_community_market(timeslot)
                    .await;
                let mut inter_community_forecasts: Vec<(String, String, Vec<ForecastSchema>)> =
                    Vec::new();

                for market in markets {
                    if seen_communities.insert(market.community_name.clone()) {
                        measurement_topologies.push(market.clone());
                    }

                    let valid_forecasts: Vec<ForecastSchema> = self
                        .demand_forecasts
                        .fetch_community_forecasts(&market, timeslot)
                        .await
                        .into_iter()
                        .filter(|forecast| self.api_adapter.validate_forecast(forecast, now))
                        .collect();

                    if valid_forecasts.is_empty() {
                        info!(
                            "No valid demand forecasts to forward for community {} (delivery {}).",
                            market.community_name, timeslot
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

                    let timeslot_forecasts: Vec<ForecastSchema> = valid_forecasts
                        .into_iter()
                        .filter(|forecast| forecast.time_slot == timeslot)
                        .collect();
                    if timeslot_forecasts.is_empty() {
                        continue;
                    }

                    if eligible_inter_community(&market.community_name) {
                        inter_community_forecasts.push((
                            market.community_name.clone(),
                            market.community_uuid.clone(),
                            timeslot_forecasts.clone(),
                        ));
                    }

                    let open_orders =
                        self.api_adapter.get_orders_for_market(&market.market_id).await;
                    let (hashes_to_delete, replacement_forecasts) =
                        plan_residual_replacement(&open_orders, &trader, timeslot_forecasts);

                    if let Err(e) =
                        remove_orders(self.gsy_node_url.clone(), hashes_to_delete, &dev::alice())
                            .await
                    {
                        error!(
                            "Failed to remove previous orders for community {}: {}",
                            market.community_name, e
                        );
                    }

                    if replacement_forecasts.is_empty() {
                        continue;
                    }

                    if let Err(e) = publish_orders(
                        self.gsy_node_url.clone(),
                        replacement_forecasts,
                        market.clone(),
                        bid_rate,
                        offer_rate,
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

                if let Some(inter_market) = inter_market {
                    self.publish_inter_community_orders(
                        &inter_market,
                        timeslot,
                        now,
                        bid_rate,
                        offer_rate,
                        &trader,
                        inter_community_forecasts,
                    )
                    .await;
                }
            }

            self.measurements
                .fetch_and_forward(measurement_topologies, now)
                .await;

            sleep(Duration::from_secs(interval_sec)).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let app_state = AppState::new();
    app_state.poll_and_forward().await;
}
