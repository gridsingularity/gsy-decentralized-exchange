use crate::constants::CommunityClientConstants;
use crate::external_forecasts::pv_pricing::effective_offer_min_rate;
use crate::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::{
    InputBid, InputOffer, InputOrder, OrderComponent,
};
use crate::time_utils::get_current_timestamp_in_secs;
use anyhow::{Error, Result};
use gsy_offchain_primitives::aggregation::{
    OrderType, RESIDUAL_ENERGY_TOLERANCE_KWH, net_to_order_type,
};
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{NODE_FLOAT_SCALING_FACTOR, string_to_h256};
use subxt::utils::H256;
use subxt::{OnlineClient, SubstrateConfig, utils::AccountId32};
use subxt_signer::sr25519::Keypair;
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

/// Linearly interpolate an order price (currency units per kWh) within the
/// `[min_rate, max_rate]` range based on how far `now` has progressed through the
/// market's `[open_time, close_time]` window.
///
/// When `increasing` is `true` (bids) the rate ramps from `min_rate` at market open
/// up to `max_rate` at market close; when `false` (offers) it ramps the other way,
/// from `max_rate` down to `min_rate`. Outside the window the rate is clamped to the
/// respective endpoint.
pub fn calculate_order_rate(
    min_rate: f64,
    max_rate: f64,
    now: u64,
    open_time: u64,
    close_time: u64,
    increasing: bool,
) -> f64 {
    let progress = if close_time <= open_time {
        1.0
    } else {
        (now.saturating_sub(open_time) as f64 / (close_time - open_time) as f64).clamp(0.0, 1.0)
    };
    if increasing {
        min_rate + progress * (max_rate - min_rate)
    } else {
        max_rate - progress * (max_rate - min_rate)
    }
}

pub async fn publish_orders(
    url: String,
    forecasts: Vec<ForecastSchema>,
    market: MarketTopologySchema,
    bid_rate: f64,
    open_time: u64,
    close_time: u64,
    signer: &Keypair,
) -> Result<(), Error> {
    let input_orders = create_input_orders(forecasts, market, bid_rate, open_time, close_time, signer);
    publish_input_orders(url, input_orders, signer).await
}

/// Sign and submit prebuilt input orders through `orderbook_worker.insert_orders`.
pub async fn publish_input_orders(
    url: String,
    input_orders: Vec<InputOrder<AccountId32>>,
    signer: &Keypair,
) -> Result<(), Error> {
    if input_orders.is_empty() {
        return Ok(());
    }
    let api = OnlineClient::<SubstrateConfig>::from_insecure_url(url).await?;

    let register_order_tx = gsy_node::tx()
        .orderbook_worker()
        .insert_orders(input_orders);

    let order_submit_and_watch = api
        .tx()
        .sign_and_submit_then_watch_default(&register_order_tx, signer)
        .await?
        .wait_for_finalized_success()
        .await?;

    let transfer_event = order_submit_and_watch
        .find_first::<gsy_node::orderbook_registry::events::AllOrdersInserted>()?;

    if let Some(event) = transfer_event {
        info!("Orders publishing success: {event:?}");
    } else {
        info!("Failed to find AllOrdersInserted Event");
    }

    Ok(())
}

/// Remove the given open orders from the chain by their on-chain order hashes.
///
/// Used to clear a trader's existing open orders for a market before publishing the
/// replacement batch, so there is a single order per trader/area instead of orders
/// stacking on top of each other. The hashes come from the off-chain order book (the
/// `_id` of each stored order equals its on-chain hash).
pub async fn remove_orders(
    url: String,
    order_hashes: Vec<H256>,
    signer: &Keypair,
) -> Result<(), Error> {
    if order_hashes.is_empty() {
        return Ok(());
    }

    let api = OnlineClient::<SubstrateConfig>::from_insecure_url(url).await?;

    let remove_order_tx = gsy_node::tx()
        .orderbook_worker()
        .remove_orders(order_hashes);

    api.tx()
        .sign_and_submit_then_watch_default(&remove_order_tx, signer)
        .await?
        .wait_for_finalized_success()
        .await?;

    Ok(())
}

fn _create_bid_object(
    forecast: ForecastSchema,
    area_info: AreaTopologySchema,
    market: MarketTopologySchema,
    energy_rate: f64,
    now: u64,
    signer: &Keypair,
) -> InputOrder<AccountId32> {
    InputOrder::Bid {
        0: InputBid {
            buyer: AccountId32::from(signer.public_key()),
            bid_component: OrderComponent {
                area_uuid: string_to_h256(area_info.area_hash.clone()),
                energy: (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
                energy_rate: (forecast.energy_kwh.abs() * energy_rate * NODE_FLOAT_SCALING_FACTOR)
                    as u64,
                market_id: string_to_h256(market.market_id.clone()),
                creation_time: now,
                time_slot: market.time_slot as u64,
            },
        },
    }
}

fn _create_offer_object(
    forecast: ForecastSchema,
    area_info: AreaTopologySchema,
    market: MarketTopologySchema,
    energy_rate: f64,
    now: u64,
    signer: &Keypair,
) -> InputOrder<AccountId32> {
    InputOrder::Offer {
        0: InputOffer {
            seller: AccountId32::from(signer.public_key()),
            offer_component: OrderComponent {
                area_uuid: string_to_h256(area_info.area_hash.clone()),
                // `forecast.energy_kwh.abs()` is the committed offer quantity: the
                // on-chain, penalty-validated number (already the conservative,
                // p5-based commitment from the PV ingestion path), not a raw point
                // forecast. There is no separate quantity stored anywhere on-chain.
                energy: (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
                energy_rate: (forecast.energy_kwh.abs() * energy_rate * NODE_FLOAT_SCALING_FACTOR)
                    as u64,
                market_id: string_to_h256(market.market_id.clone()),
                creation_time: now,
                time_slot: market.time_slot as u64,
            },
        },
    }
}

/// Build at most one aggregated inter-community order from a community's net import.
/// `net_import_kwh > 0` (deficit) yields a Bid, `< 0` (surplus) an Offer, a tie none.
/// `area_uuid` carries the `community_id` hash; `market_id` is the reserved
/// inter-community market id for the timeslot.
pub fn create_inter_community_order(
    net_import_kwh: f64,
    community_id: H256,
    market_id: H256,
    time_slot: u64,
    rate: f64,
    signer: &Keypair,
) -> Option<InputOrder<AccountId32>> {
    let component = OrderComponent {
        area_uuid: community_id,
        energy: (net_import_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (net_import_kwh.abs() * rate * NODE_FLOAT_SCALING_FACTOR) as u64,
        market_id,
        creation_time: get_current_timestamp_in_secs(),
        time_slot,
    };
    match net_to_order_type(net_import_kwh, RESIDUAL_ENERGY_TOLERANCE_KWH) {
        OrderType::Bid => Some(InputOrder::Bid(InputBid {
            buyer: AccountId32::from(signer.public_key()),
            bid_component: component,
        })),
        OrderType::Offer => Some(InputOrder::Offer(InputOffer {
            seller: AccountId32::from(signer.public_key()),
            offer_component: component,
        })),
        OrderType::None => None,
    }
}

/// Turn forecasts into signed input orders.
pub fn create_input_orders(
    forecasts: Vec<ForecastSchema>,
    market: MarketTopologySchema,
    bid_rate: f64,
    open_time: u64,
    close_time: u64,
    signer: &Keypair,
) -> Vec<InputOrder<AccountId32>> {
    let now: u64 = get_current_timestamp_in_secs();

    let mut input_orders: Vec<InputOrder<AccountId32>> = Vec::new();

    for forecast in forecasts {
        let area_info = market
            .community_areas
            .iter()
            .find(|area| area.area_hash == forecast.area_hash);
        if area_info.is_none() {
            continue;
        }

        if forecast.energy_kwh > 0. {
            input_orders.push(_create_bid_object(
                forecast,
                area_info.unwrap().clone(),
                market.clone(),
                bid_rate,
                now,
                signer,
            ));
        } else if forecast.energy_kwh < 0. {
            // Per-forecast offer rate: ramp MAX_ORDER_RATE -> effective floor, where the
            // floor is lifted for low-confidence forecasts. confidence == 1.0 reproduces
            // the pre-change ramp (MAX -> MIN_ORDER_RATE).
            let effective_min = effective_offer_min_rate(
                CommunityClientConstants.MIN_ORDER_RATE,
                CommunityClientConstants.MAX_ORDER_RATE,
                forecast.confidence,
                CommunityClientConstants.PV_PRICE_CONFIDENCE_WEIGHT,
            );
            let offer_rate = calculate_order_rate(
                effective_min,
                CommunityClientConstants.MAX_ORDER_RATE,
                now,
                open_time,
                close_time,
                false,
            );
            input_orders.push(_create_offer_object(
                forecast,
                area_info.unwrap().clone(),
                market.clone(),
                offer_rate,
                now,
                signer,
            ));
        }
    }
    input_orders
}
