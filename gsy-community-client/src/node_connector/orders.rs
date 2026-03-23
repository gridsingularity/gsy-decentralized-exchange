use crate::time_utils::get_current_timestamp_in_secs;
use anyhow::{Error, Result};
use ethers::prelude::*;
use ethers::utils::keccak256;
use gsy_offchain_primitives::db_api_schema::market::{AreaTopologySchema, MarketTopologySchema};
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::NODE_FLOAT_SCALING_FACTOR;
use std::str::FromStr;
use tracing::{info, warn};

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

pub type EvmOrderParamsTuple = (Address, u64, [u8; 32], [u8; 32], u64, u64, u64, u64, bool);

pub async fn publish_orders(
    evm_node_url: String,
    forecasts: Vec<ForecastSchema>,
    market: MarketTopologySchema,
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

    let input_orders = create_input_orders(forecasts, market, signer_address);
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
                        {"name": "owner", "type": "address"},
                        {"name": "nonce", "type": "uint64"},
                        {"name": "areaUuid", "type": "bytes32"},
                        {"name": "marketId", "type": "bytes32"},
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

fn parse_or_hash_bytes32(value: &str) -> [u8; 32] {
    if value.starts_with("0x") && value.len() == 66 {
        if let Ok(parsed) = H256::from_str(value) {
            return parsed.to_fixed_bytes();
        }
    }
    keccak256(value.as_bytes())
}

fn build_order_param(
    forecast: &ForecastSchema,
    area_info: &AreaTopologySchema,
    market: &MarketTopologySchema,
    now: u64,
    owner: Address,
    nonce: u64,
    is_bid: bool,
) -> EvmOrderParamsTuple {
    let rate_multiplier = if is_bid { BID_RATE } else { OFFER_RATE };
    (
        owner,
        nonce,
        parse_or_hash_bytes32(area_info.area_uuid.as_str()),
        parse_or_hash_bytes32(market.market_id.as_str()),
        market.time_slot as u64,
        now,
        (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
        (forecast.energy_kwh.abs() * rate_multiplier * NODE_FLOAT_SCALING_FACTOR) as u64,
        is_bid,
    )
}

pub fn create_input_orders(
    forecasts: Vec<ForecastSchema>,
    market: MarketTopologySchema,
    owner: Address,
) -> Vec<EvmOrderParamsTuple> {
    let now: u64 = get_current_timestamp_in_secs();

    let mut input_orders = Vec::new();

    for (index, forecast) in forecasts.into_iter().enumerate() {
        let area_info = market
            .community_areas
            .iter()
            .find(|area| area.area_uuid == forecast.area_uuid)
            .cloned();
        if area_info.is_none() {
            continue;
        }
        let area_info = area_info.unwrap();
        let nonce = now.saturating_add(index as u64);

        if forecast.energy_kwh > 0. {
            input_orders.push(build_order_param(
                &forecast, &area_info, &market, now, owner, nonce, true,
            ));
        } else if forecast.energy_kwh < 0. {
            input_orders.push(build_order_param(
                &forecast, &area_info, &market, now, owner, nonce, false,
            ));
        }
    }
    input_orders
}
