use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_measurements::manager::MeasurementsManager;
use gsy_community_client::inter_community::eligible_inter_community;
use gsy_community_client::node_connector::orders::{
    calculate_order_rate, create_inter_community_order, publish_input_orders, publish_orders,
    remove_orders,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    AreaMarketInfoAdapter, deterministic_area_hash, deterministic_area_uuid,
    deterministic_community_uuid, plan_residual_replacement,
};
use gsy_community_client::time_utils::{
    get_current_timestamp_in_secs, open_spot_market_timeslots, start_of_previous_day,
};
use gsy_community_client::topology::TopologyManager;
use gsy_offchain_primitives::aggregation::aggregate_net_import;
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{
    community_id_from_uuid, h256_to_string, read_env_or, string_to_h256,
};
use reqwest::Client;
use std::collections::{HashMap, HashSet};
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
    forecasts_manager: ForecastsManager,
    gsy_node_url: String,
}

impl AppState {
    fn new() -> Self {
        let api_adapter = AreaMarketInfoAdapter::new(Some(read_env_or(
            "OFFCHAIN_STORAGE_URL",
            "http://gsy-orderbook:8080".to_string(),
        )));
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
            forecasts_manager: ForecastsManager::new(),
            // subxt's default transport (jsonrpsee) is WebSocket-only and rejects any
            // scheme other than ws/wss, so this must stay a `ws://` URL.
            gsy_node_url: read_env_or("GSY_NODE_URL", "ws://gsy-node:9944".to_string()),
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
            // PV production forecasts now flow into the community forecast vec as
            // negative `energy_kwh`, so this genuinely nets production against
            // consumption per community/timeslot (surplus -> offer, deficit -> bid).
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

    /// Day-ahead forecast ingestion loop. Every `FORECAST_INGEST_INTERVAL_SEC`, asks the
    /// forecasters for the rolling 48h window starting at yesterday's midnight and upserts
    /// every returned point to storage. This is the *only* writer of `/forecasts`; it never
    /// builds or publishes orders, so a forecaster outage here does not block the
    /// publish loop from re-publishing whatever was already ingested.
    async fn ingest_forecasts_loop(&self) {
        let interval_sec = CommunityClientConstants.FORECAST_INGEST_INTERVAL_SEC.max(1);
        let horizon_sec = CommunityClientConstants.FORECAST_INGEST_HORIZON_SEC;

        loop {
            let now = get_current_timestamp_in_secs();
            let start_time = start_of_previous_day(now);

            let communities = TopologyManager::new(&self.client, &self.api_adapter)
                .fetch_all_topology()
                .await;

            for community in communities {
                let community_uuid = deterministic_community_uuid(&community.community_name);
                let areas: Vec<AreaTopologySchema> = community
                    .areas
                    .iter()
                    .map(|area| AreaTopologySchema {
                        area_uuid: deterministic_area_uuid(
                            &community.community_name,
                            &area.area_name,
                        ),
                        area_type: area.area_type.clone(),
                        name: area.area_name.clone(),
                        area_hash: h256_to_string(deterministic_area_hash(
                            &community.community_name,
                            &area.area_name,
                        )),
                    })
                    .collect();

                let forecasts = self
                    .forecasts_manager
                    .fetch_area_set_forecasts(
                        &community_uuid,
                        &community.community_name,
                        &areas,
                        start_time,
                    )
                    .await;

                // Keep every future point the forecaster returns; unlike the publish loop's
                // `validate_forecast`, do NOT also drop non-future slots here — ingestion
                // re-runs hourly and must keep persisting today's remaining slots too.
                let to_store: Vec<ForecastSchema> = forecasts
                    .into_iter()
                    .filter(|forecast| forecast.energy_kwh != 0.0)
                    .collect();

                if to_store.is_empty() {
                    continue;
                }

                info!(
                    "Ingesting {} forecast point(s) for community {} (window start {}, horizon {}s).",
                    to_store.len(),
                    community.community_name,
                    start_time,
                    horizon_sec
                );

                if let Err(e) = self.api_adapter.forward_forecast(to_store).await {
                    error!(
                        "Failed to ingest forecasts for community {}: {}",
                        community.community_name, e
                    );
                }
            }

            sleep(Duration::from_secs(interval_sec)).await;
        }
    }

    /// Order-publication loop. Every `ORDER_RESUBMISSION_INTERVAL_SEC`, reads forecasts back
    /// from storage (never from the forecasters) for every currently open market slot and
    /// (re)publishes bids/offers from them, so order publication survives forecaster
    /// downtime as long as ingestion previously wrote something for that slot.
    async fn publish_orders_loop(&self) {
        let interval_sec = CommunityClientConstants
            .ORDER_RESUBMISSION_INTERVAL_SEC
            .max(1);

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
            let window_start = *open_timeslots.iter().min().expect("non-empty");
            let window_end = *open_timeslots.iter().max().expect("non-empty");

            let markets_per_timeslot = TopologyManager::new(&self.client, &self.api_adapter)
                .get_for_timeslots(&open_timeslots)
                .await;

            let mut measurement_topologies: Vec<MarketTopologySchema> = Vec::new();
            let mut seen_communities: HashSet<String> = HashSet::new();
            // Fetched once per community per tick, spanning every open timeslot in
            // `[window_start, window_end]`, and reused across every timeslot iteration below
            // instead of one GET per (community, timeslot).
            let mut forecasts_by_community: HashMap<String, Vec<ForecastSchema>> = HashMap::new();

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

                    let community_forecasts =
                        match forecasts_by_community.get(&market.community_uuid) {
                            Some(cached) => cached.clone(),
                            None => {
                                let fetched = self
                                    .api_adapter
                                    .get_forecasts_for_community(
                                        &market.community_uuid,
                                        window_start,
                                        window_end,
                                    )
                                    .await;
                                forecasts_by_community
                                    .insert(market.community_uuid.clone(), fetched.clone());
                                fetched
                            }
                        };

                    let timeslot_forecasts: Vec<ForecastSchema> = community_forecasts
                        .iter()
                        .filter(|forecast| {
                            forecast.time_slot == timeslot
                                && self.api_adapter.validate_forecast(forecast, now)
                        })
                        .cloned()
                        .collect();

                    if timeslot_forecasts.is_empty() {
                        info!(
                            "No valid stored forecasts to publish for community {} (delivery {}).",
                            market.community_name, timeslot
                        );
                        continue;
                    }

                    if eligible_inter_community(&market.community_name) {
                        inter_community_forecasts.push((
                            market.community_name.clone(),
                            market.community_uuid.clone(),
                            timeslot_forecasts.clone(),
                        ));
                    }

                    let open_orders = self
                        .api_adapter
                        .get_orders_for_market(&market.market_id)
                        .await;
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
                        open_time,
                        close_time,
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
    let ingest_state = app_state.clone();
    let publish_state = app_state.clone();

    // Two independent, never-returning loops. Each runs in its own task so a panic or
    // stall in one (e.g. the ingestion loop wedged on a downed forecaster) cannot block
    // the other (order publication, which only depends on storage).
    let ingest_handle = tokio::spawn(async move { ingest_state.ingest_forecasts_loop().await });
    let publish_handle = tokio::spawn(async move { publish_state.publish_orders_loop().await });

    let _ = tokio::join!(ingest_handle, publish_handle);
}
