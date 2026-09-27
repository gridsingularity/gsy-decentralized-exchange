use gsy_community_client::asset_did::{AssetDidClient, build_sync_payload};
use gsy_community_client::constants::CommunityClientConstants;
use gsy_community_client::external_forecasts::manager::ForecastsManager;
use gsy_community_client::external_measurements::manager::{
    MeasurementsManager, describe_metering_points,
};
use gsy_community_client::external_measurements::metering_points::{
    MeteringPointSet, build_metering_points, parse_overrides,
};
use gsy_community_client::inter_community::{demand_coverage, eligible_inter_community};
use gsy_community_client::node_connector::orders::{
    calculate_order_rate, create_inter_community_order, publish_input_orders, publish_orders,
    remove_orders,
};
use gsy_community_client::offchain_storage_connector::adapter::{
    AreaMarketInfoAdapter, deterministic_areas, deterministic_community_uuid,
    plan_residual_replacement,
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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use subxt::utils::AccountId32;
use subxt_signer::sr25519::dev;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Clone)]
struct AppState {
    client: Client,
    api_adapter: AreaMarketInfoAdapter,
    measurements: MeasurementsManager,
    forecasts_manager: ForecastsManager,
    asset_did_client: AssetDidClient,
    gsy_node_url: String,
    /// The latest metering points, written by the measurement loop and read by the publish
    /// loop for the inter-community demand guard. `None` until the first set is built.
    metering_points: Arc<RwLock<Option<MeteringPointSet>>>,
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
            // Reads `IDENTITY_SERVER_URL` and the shared `API_KEY` itself; both have
            // defaults, so this never fails to construct.
            asset_did_client: AssetDidClient::new(None, None),
            // subxt's default transport (jsonrpsee) is WebSocket-only and rejects any
            // scheme other than ws/wss, so this must stay a `ws://` URL.
            gsy_node_url: read_env_or("GSY_NODE_URL", "ws://gsy-node:9944".to_string()),
            metering_points: Arc::new(RwLock::new(None)),
        }
    }

    /// A copy of the latest metering point set, if any. The lock is released on return, so
    /// callers never hold it across an `.await`.
    fn metering_point_set(&self) -> Option<MeteringPointSet> {
        match self.metering_points.read() {
            Ok(set) => set.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
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
                // Same derivation the market topology uses (`build_new_market_topology`),
                // so an ingested forecast's `area_uuid`/`area_hash` always join to the
                // market area for the same `(community_name, asset_name)` pair.
                let areas: Vec<AreaTopologySchema> = deterministic_areas(&community);

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

    /// Asset-DID sync loop. Every `ASSET_DID_SYNC_INTERVAL_SEC`, re-reads the ontology and
    /// pushes every community and asset subject to the identity server, which derives a
    /// `did:ethr` per subject from its master seed and upserts one record each. The sync is
    /// idempotent: a tick that finds no ontology change creates nothing and returns
    /// byte-identical DIDs.
    ///
    /// Deliberately a separate loop rather than a step inside `ingest_forecasts_loop` (plan
    /// §2.4): identities are a side channel, and an identity server that is down, slow, or
    /// 500ing must not delay a single forecast or order. Nothing in here propagates an error
    /// or panics — every failure is logged and the loop waits for the next tick.
    async fn sync_asset_dids_loop(&self) {
        let interval_sec = CommunityClientConstants.ASSET_DID_SYNC_INTERVAL_SEC.max(1);

        loop {
            // DIDs stay keyed on the ontology LEC (`Pilot1`, ...), not on the per-site
            // communities the markets use: switching would re-issue every community and
            // asset DID and leave the old ones un-retired, since the identity server only
            // retires subjects within the communities a payload carries. So the subject ids
            // stay `deterministic_area_uuid(<LEC>, <asset>)` and differ from market area ids.
            //
            // `fetch_all_topology_by_lec` logs its own request failures instead of returning
            // them: an unreachable ontology gives an empty vec, and a LEC whose asset query
            // fails is left out (which, for the same reason, retires nothing). Running it as a
            // child task still turns any panic in it into a `JoinError` this loop can log and
            // retry on the next tick, instead of killing the sync permanently.
            let fetch_state = self.clone();
            let fetched = tokio::spawn(async move {
                TopologyManager::new(&fetch_state.client, &fetch_state.api_adapter)
                    .fetch_all_topology_by_lec()
                    .await
            })
            .await;

            let communities = match fetched {
                Ok(communities) => communities,
                Err(error) => {
                    error!(
                        "Asset DID sync: fetching the external topology panicked ({}); \
                         retrying at the next tick.",
                        error
                    );
                    sleep(Duration::from_secs(interval_sec)).await;
                    continue;
                }
            };

            let payload = build_sync_payload(&communities);

            if payload.is_empty() {
                // Either the ontology is unreachable or it returned nothing. Do not post:
                // the server rejects an empty payload, and an empty payload could never be
                // meant as "retire everything" anyway.
                info!("Asset DID sync: no communities or assets to sync; skipping this tick.");
            } else {
                match self.asset_did_client.sync(&payload).await {
                    Ok(response) => {
                        info!(
                            "Asset DID sync: {} subject(s) sent ({} communities, {} assets); \
                             created {}, updated {}, retired {}.",
                            payload.subject_count(),
                            payload.communities.len(),
                            payload.assets.len(),
                            response.created,
                            response.updated,
                            response.retired,
                        );
                    }
                    Err(error) => {
                        // Logged and dropped on purpose: the forecast and order loops run in
                        // their own tasks and neither reads anything this loop writes.
                        error!(
                            "Asset DID sync of {} subject(s) failed: {:#}",
                            payload.subject_count(),
                            error
                        );
                    }
                }
            }

            sleep(Duration::from_secs(interval_sec)).await;
        }
    }

    /// Measurement loop. Every `MEASUREMENT_INGEST_INTERVAL_SEC`, rebuilds the metering
    /// points from the ontology, shares them with the publish loop, reads InfluxDB over the
    /// look-back window and forwards one row per point and ended slot to storage. Runs in
    /// its own task, so a slow InfluxDB or storage never delays order publication. Nothing
    /// in here propagates an error or panics: every failure is logged and the loop waits for
    /// the next tick.
    async fn ingest_measurements_loop(&self) {
        let interval_sec = CommunityClientConstants
            .MEASUREMENT_INGEST_INTERVAL_SEC
            .max(1);
        let overrides = match parse_overrides(&CommunityClientConstants.METERING_POINT_OVERRIDES)
        {
            Ok(overrides) => overrides,
            Err(error) => {
                error!("Ignoring METERING_POINT_OVERRIDES: {}", error);
                HashMap::new()
            }
        };
        if !overrides.is_empty() {
            info!("Metering point overrides: {:?}", overrides);
        }

        let mut current_set: Option<MeteringPointSet> = None;
        let mut last_unmapped: Option<BTreeSet<String>> = None;

        loop {
            // Fetched in a child task, as in `sync_asset_dids_loop`, so a panic in it is a
            // `JoinError` logged here rather than the end of the loop.
            let fetch_state = self.clone();
            let fetch_overrides = overrides.clone();
            let built = tokio::spawn(async move {
                let raw = TopologyManager::new(&fetch_state.client, &fetch_state.api_adapter)
                    .fetch_raw_ontology()
                    .await
                    .map_err(|error| error.to_string())?;
                let set = build_metering_points(&raw.buildings, &raw.assets, &fetch_overrides);
                Ok::<_, String>((set, raw.missing_lecs()))
            })
            .await;

            let fetched_set = match built {
                Ok(Ok((set, missing_lecs))) if missing_lecs.is_empty() => Some(set),
                Ok(Ok((set, missing_lecs))) => {
                    // A LEC's assets failed to load: its sites would lose every point.
                    // Keep the last complete set; with none yet, use what loaded.
                    error!(
                        "Measurements: the assets of {} failed to load; {}",
                        missing_lecs.join(", "),
                        if current_set.is_some() {
                            "reusing the last metering points"
                        } else {
                            "using the metering points of the other LECs"
                        }
                    );
                    current_set.is_none().then_some(set)
                }
                Ok(Err(error)) => {
                    error!("Measurements: fetching the ontology failed: {}", error);
                    None
                }
                Err(error) => {
                    error!("Measurements: fetching the ontology panicked: {}", error);
                    None
                }
            };

            if let Some(set) = fetched_set {
                if current_set.as_ref() != Some(&set) {
                    for note in &set.notes {
                        info!("Metering points: {}", note);
                    }
                    for line in describe_metering_points(&set) {
                        info!("Metering points of {}", line);
                    }
                    match self.metering_points.write() {
                        Ok(mut shared) => *shared = Some(set.clone()),
                        Err(poisoned) => *poisoned.into_inner() = Some(set.clone()),
                    }
                    current_set = Some(set);
                }
            } else if current_set.is_some() {
                info!("Measurements: reusing the last metering points.");
            }

            let Some(set) = current_set.clone() else {
                info!("Measurements: no metering points built yet; skipping this tick.");
                sleep(Duration::from_secs(interval_sec)).await;
                continue;
            };

            let manager = self.measurements.clone();
            let now = get_current_timestamp_in_secs();
            let report = match tokio::spawn(async move { manager.ingest(&set, now).await }).await
            {
                Ok(report) => report,
                Err(error) => {
                    error!("Measurements: the ingest panicked: {}", error);
                    sleep(Duration::from_secs(interval_sec)).await;
                    continue;
                }
            };

            if report.no_readings {
                error!(
                    "Measurements: InfluxDB returned no readings for the last {}s; nothing \
                     forwarded this tick.",
                    CommunityClientConstants.MEASUREMENT_LOOKBACK_SEC
                );
            } else {
                for (community, counts) in &report.summary.counts {
                    info!(
                        "Measurements of {}: {} complete, {} incomplete, {} missing row(s) \
                         forwarded.",
                        community, counts.complete, counts.incomplete, counts.missing
                    );
                }
                for row in &report.summary.incomplete {
                    warn!(
                        "Measurements: {} ({}) is incomplete for slot {}; missing meter(s) {}.",
                        row.point,
                        row.community,
                        row.time_slot,
                        row.missing_meters.join(", ")
                    );
                }
            }
            if last_unmapped.as_ref() != Some(&report.unmapped_tokens) {
                if !report.unmapped_tokens.is_empty() {
                    info!(
                        "Measurements: InfluxDB meter(s) without a SmartMeter in the ontology, \
                         ignored: {}",
                        report
                            .unmapped_tokens
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                last_unmapped = Some(report.unmapped_tokens.clone());
            }
            if let Some(error) = &report.forward_error {
                error!(
                    "Measurements: forwarding {} row(s) failed: {}",
                    report.forwarded, error
                );
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
        // Communities not on the inter-community list that have already been reported as
        // ready for it, so the readiness line is logged once per run.
        let mut reported_ready: HashSet<String> = HashSet::new();

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

            // The inter-community demand guard needs each community's expected SmartMeters;
            // until the measurement loop has built them, no inter-community order is sent.
            let metering_points = self.metering_point_set();
            if metering_points.is_none() {
                info!(
                    "No metering points built yet; publishing no inter-community orders this tick."
                );
            }
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

                    // A community's net order only means something if its whole demand is
                    // forecast: an eligible community joins the inter-community market for
                    // the slot only with a demand forecast for every expected SmartMeter.
                    if let Some(set) = &metering_points {
                        let coverage =
                            demand_coverage(set, &market.community_name, &timeslot_forecasts);
                        if eligible_inter_community(&market.community_name) {
                            if coverage.is_full() {
                                inter_community_forecasts.push((
                                    market.community_name.clone(),
                                    market.community_uuid.clone(),
                                    timeslot_forecasts.clone(),
                                ));
                            } else {
                                info!(
                                    "Community {} stays out of the inter-community market for \
                                     delivery {}: {} of its {} expected SmartMeter(s) have no \
                                     demand forecast.",
                                    market.community_name,
                                    timeslot,
                                    coverage.missing.len(),
                                    coverage.expected
                                );
                            }
                        } else if coverage.is_full()
                            && reported_ready.insert(market.community_name.clone())
                        {
                            info!(
                                "Community {} has demand forecasts for all {} expected \
                                 SmartMeter(s) (delivery {}); it can be added to \
                                 INTER_COMMUNITY_ELIGIBLE_COMMUNITIES.",
                                market.community_name, coverage.expected, timeslot
                            );
                        }
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

            sleep(Duration::from_secs(interval_sec)).await;
        }
    }
}

#[tokio::main]
async fn main() {
    // Without this every `info!`/`error!` below is discarded: `tracing` drops events when no
    // subscriber is installed, so the service ran completely silently and a forecaster or
    // InfluxDB failure left no trace at all. Same idiom as the market orchestrator, so
    // RUST_LOG drives the level.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting GSY Community Client...");
    let app_state = AppState::new();
    let ingest_state = app_state.clone();
    let publish_state = app_state.clone();
    let asset_did_state = app_state.clone();
    let measurement_state = app_state.clone();

    // Four independent, never-returning loops. Each runs in its own task so a panic or
    // stall in one (e.g. the ingestion loop wedged on a downed forecaster, or the DID sync
    // waiting on an unreachable identity server) cannot block the others. They share only
    // the HTTP clients and the latest metering points, which the measurement loop writes
    // and the publish loop reads for its inter-community demand guard.
    let ingest_handle = tokio::spawn(async move { ingest_state.ingest_forecasts_loop().await });
    let publish_handle = tokio::spawn(async move { publish_state.publish_orders_loop().await });
    let asset_did_handle =
        tokio::spawn(async move { asset_did_state.sync_asset_dids_loop().await });
    let measurement_handle =
        tokio::spawn(async move { measurement_state.ingest_measurements_loop().await });

    let _ = tokio::join!(
        ingest_handle,
        publish_handle,
        asset_did_handle,
        measurement_handle
    );
}
