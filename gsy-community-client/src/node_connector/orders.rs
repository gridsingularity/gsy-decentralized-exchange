use crate::time_utils::get_current_timestamp_in_secs;
use anyhow::{Error, Result};
use ethers::prelude::*;
use primitives::db_api_schema::{
    market::MarketSchema,
    profiles::ForecastSchema,
    ids::IdType
};
use primitives::ewds::get_onchain_id;
use primitives::utils::{string_to_timestamp, NODE_FLOAT_SCALING_FACTOR};
use std::str::FromStr;
use tracing::{info, warn};
use uuid::Uuid;

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

pub type EvmOrderParamsTuple = ([u8; 16], [u8; 16], [u8; 16], u64, u64, u64, u64, bool);

pub async fn publish_orders(
    evm_node_url: String,
    forecasts: Vec<ForecastSchema>,
    market: MarketSchema,
    order_registry_address: String,
    community_signer_private_key: String,
) -> Result<(), Error> {
    let order_registry_address = Address::from_str(order_registry_address.as_str())
        .map_err(|e| anyhow::anyhow!("Invalid order registry address: {}", e))?;
    if order_registry_address.is_zero() {
        warn!(
            "ORDER_REGISTRY_ADDRESS is zero; placeOrder transactions will fail until configured."
        );
    }

    let provider = Provider::<Ws>::connect(evm_node_url.as_str()).await?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = community_signer_private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow::anyhow!("Invalid community client private key: {}", e))?
        .with_chain_id(chain_id);
    let signer_address = wallet.address();

    let input_orders = create_input_orders(forecasts, market, signer_address).await?;
    if input_orders.is_empty() {
        info!("No orders to publish for this cycle");
        return Ok(());
    }

    let client = std::sync::Arc::new(SignerMiddleware::new(provider, wallet));
    let order_registry = OrderRegistryContract::new(order_registry_address, client.clone());

    info!("Publishing {} orders to OrderRegistry", input_orders.len());
    for (index, input_order) in input_orders.into_iter().enumerate() {
        let place_order_call = order_registry.place_order(input_order);
        let pending_tx = place_order_call.send().await?;
        let tx_hash = pending_tx.tx_hash();
        let receipt = pending_tx.await?;

        match receipt {
            Some(receipt) => {
                if receipt
                    .status
                    .map(|status| status.as_u64())
                    .unwrap_or_default()
                    != 1
                {
                    return Err(anyhow::anyhow!(
                        "placeOrder tx {} ({:?}) reverted with status {:?}",
                        index,
                        tx_hash,
                        receipt.status
                    ));
                }
                info!("Order {} published successfully. tx={:?}", index, tx_hash);
            }
            None => {
                return Err(anyhow::anyhow!(
                    "placeOrder tx {} ({:?}) dropped without receipt",
                    index,
                    tx_hash
                ));
            }
        }
    }

    Ok(())
}

abigen!(
    OrderRegistryContract,
    r#"[
        {
            "type": "function",
            "name": "placeOrder",
            "stateMutability": "nonpayable",
            "inputs": [
                {
                    "name": "params",
                    "type": "tuple",
                    "components": [
                        {"name": "orderId", "type": "bytes16"},
                        {"name": "createdBy", "type": "bytes16"},
                        {"name": "marketId", "type": "bytes16"},
                        {"name": "timeSlot", "type": "uint64"},
                        {"name": "creationTime", "type": "uint64"},
                        {"name": "energy", "type": "uint64"},
                        {"name": "energyRate", "type": "uint64"},
                        {"name": "isBid", "type": "bool"}
                    ]
                }
            ],
            "outputs": []
        }
    ]"#
);

async fn build_order_param(
    forecast: &ForecastSchema,
    facility_id: &String,
    market: &MarketSchema,
    now: u64,
    is_bid: bool,
) -> Result<EvmOrderParamsTuple> {
    let rate_multiplier = if is_bid { BID_RATE } else { OFFER_RATE };
    let onchain_order_id = get_onchain_id(
        Uuid::new_v4().to_string(),
        IdType::OrderId
    ).await?;
    let onchain_facility_id = get_onchain_id(
        facility_id.to_string(),
        IdType::ActorId
    ).await?;
    let onchain_market_id = get_onchain_id(
        market.market_id.to_string(),
        IdType::ActorId
    ).await?;
    let delivery_start: u64 =
        string_to_timestamp(&market.delivery_start_time).expect("invalid delivery_start_time");
    Ok((
        onchain_order_id,
        onchain_facility_id,
        onchain_market_id,
        delivery_start,
        now,
        (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
        (forecast.energy_kwh.abs() * rate_multiplier * NODE_FLOAT_SCALING_FACTOR) as u64,
        is_bid,
    ))
}

pub async fn create_input_orders(
    forecasts: Vec<ForecastSchema>,
    market: MarketSchema,
    owner: Address,
) -> Result<Vec<EvmOrderParamsTuple>> {
    let now: u64 = get_current_timestamp_in_secs();
    let _owner = owner;

    let mut input_orders = Vec::new();

    for forecast in forecasts.into_iter() {
        if forecast.energy_kwh > 0. {
            input_orders.push(build_order_param(
                &forecast,
                &forecast.facility_id,
                &market,
                now,
                true,
            ).await?);
        } else if forecast.energy_kwh < 0. {
            input_orders.push(build_order_param(
                &forecast,
                &forecast.facility_id,
                &market,
                now,
                false,
            ).await?);
        }
    }
    Ok(input_orders)
}
