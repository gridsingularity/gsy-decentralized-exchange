use crate::order_events::publish_rejection;
use crate::time_utils::get_current_timestamp_in_secs;
use anyhow::{Context, Error, Result};
use ethers::prelude::*;
use primitives::db_api_schema::grid_topology::FacilityMarketValidationRequest;
use primitives::db_api_schema::market::MarketSchema;
use primitives::db_api_schema::orders::order_metadata_to_contract;
use primitives::db_api_schema::profiles::ForecastSchema;
use primitives::ewds::dto::EwdsOrderDto;
use primitives::ewds::{env_var, EwdsClient};
use primitives::offchain_storage::OffchainStorageClient;
use primitives::utils::{
    create_encrypted_bytes16_from_string, epoch_to_rfc3339, parse_uuid_or_hex_bytes16,
    string_to_timestamp, NODE_FLOAT_SCALING_FACTOR,
};

use std::str::FromStr;
use tracing::{info, warn};
use uuid::Uuid;

const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

pub type EvmOrderParamsTuple = (
    [u8; 16],
    [u8; 16],
    [u8; 16],
    u64,
    u64,
    u64,
    u64,
    u8,
    u8,
    bool,
    [u8; 16],
    u64,
);

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

    let input_orders = create_input_orders(forecasts, market).await?;

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
                        {"name": "energySourcePreference", "type": "uint8"},
                        {"name": "energyType", "type": "uint8"},
                        {"name": "isBid", "type": "bool"},
                        {"name": "preferredTradingPartner", "type": "bytes16"},
                        {"name": "preferredEnergyRate", "type": "uint64"}
                    ]
                }
            ],
            "outputs": []
        },
        {
            "type": "function",
            "name": "getStatus",
            "stateMutability": "view",
            "inputs": [{"name": "orderId", "type": "bytes16"}],
            "outputs": [{"name": "", "type": "uint8"}]
        },
        {
            "type": "function",
            "name": "hasRole",
            "stateMutability": "view",
            "inputs": [{"name": "role", "type": "bytes32"}, {"name": "account", "type": "address"}],
            "outputs": [{"name": "", "type": "bool"}]
        },
        {"type": "error", "name": "MarketClosed", "inputs": []},
        {"type": "error", "name": "Unauthorized", "inputs": []},
        {"type": "error", "name": "InvalidOrderParams", "inputs": []},
        {"type": "error", "name": "OrderNotOpen", "inputs": []},
        {"type": "error", "name": "OrderAlreadyExists", "inputs": []}
    ]"#
);

async fn build_order_param(
    forecast: &ForecastSchema,
    storage: &OffchainStorageClient,
    market: &MarketSchema,
    now: u64,
    is_bid: bool,
) -> Result<EvmOrderParamsTuple> {
    let rate_multiplier = if is_bid { BID_RATE } else { OFFER_RATE };
    let metadata = order_metadata_to_contract(None, None)?;
    let offchain_order_id = Uuid::new_v4().to_string();
    let onchain_order_id = create_encrypted_bytes16_from_string(&offchain_order_id);
    let facility_id = storage.fetch_onchain_id(&forecast.facility_id).await?;
    let onchain_facility_id =
        parse_uuid_or_hex_bytes16(&facility_id).context("Invalid on-chain facility ID")?;
    let delivery_start: u64 =
        string_to_timestamp(&market.delivery_start_time).expect("invalid delivery_start_time");
    Ok((
        onchain_order_id,
        onchain_facility_id,
        parse_uuid_or_hex_bytes16(market.market_id.as_str())
            .expect("could not convert hex to bytes"),
        delivery_start,
        now,
        (forecast.energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
        (forecast.energy_kwh.abs() * rate_multiplier * NODE_FLOAT_SCALING_FACTOR) as u64,
        metadata.energy_source_preference,
        metadata.energy_type,
        is_bid,
        metadata.preferred_trading_partner,
        metadata.preferred_energy_rate,
    ))
}

/// Validate non-zero forecasts before resolving facility IDs or submitting any orders.
/// Denials are skipped and reported when EWDS is enabled; infrastructure errors propagate.
pub async fn create_input_orders(
    forecasts: Vec<ForecastSchema>,
    market: MarketSchema,
) -> Result<Vec<EvmOrderParamsTuple>> {
    let now: u64 = get_current_timestamp_in_secs();

    let storage = OffchainStorageClient::from_env("EWDS_COMMUNITY_CLIENT_ID", "gsycommunityclient");
    let forecasts: Vec<_> = forecasts
        .into_iter()
        .filter(|forecast| forecast.energy_kwh > 0.0 || forecast.energy_kwh < 0.0)
        .collect();
    let requests: Vec<_> = forecasts
        .iter()
        .map(|forecast| FacilityMarketValidationRequest {
            facility_id: forecast.facility_id.clone(),
            market_id: market.market_id.clone(),
        })
        .collect();
    let validation = storage
        .validate_facility_markets(&requests)
        .await
        .context("Facility participation validation failed")?;
    let mut input_orders = Vec::new();
    let mut rejected = Vec::new();
    for (forecast, result) in forecasts.into_iter().zip(validation) {
        if let Some(reason) = result.reason {
            let reason = format!(
                "Facility {} cannot participate in market {}: {}",
                forecast.facility_id, market.market_id, reason.as_str()
            );
            warn!("{}", reason);
            rejected.push((forecast, reason));
        } else {
            input_orders.push(
                build_order_param(&forecast, &storage, &market, now, forecast.energy_kwh > 0.0)
                    .await?,
            );
        }
    }

    // Finish preparing eligible orders first: an infrastructure failure is not a rejection.
    if env_var("EWDS_ENABLE_HANDLER")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
        && !rejected.is_empty()
    {
        let ewds = EwdsClient::from_env("EWDS_COMMUNITY_CLIENT_ID", "gsycommunityclient", 60_000);
        let delivery_start = string_to_timestamp(&market.delivery_start_time)
            .context("Invalid delivery_start_time")?;
        for (forecast, reason) in rejected {
            let is_bid = forecast.energy_kwh > 0.0;
            let submission = EwdsOrderDto {
                order_id: Uuid::new_v4().to_string(),
                market_id: market.market_id.clone(),
                order_type: if is_bid { "bid" } else { "offer" }.to_string(),
                order_status: "submitted".to_string(),
                time_slot: epoch_to_rfc3339(delivery_start),
                quantity: forecast.energy_kwh.abs(),
                price_limit: forecast.energy_kwh.abs() * if is_bid { BID_RATE } else { OFFER_RATE },
                created_by: forecast.facility_id,
                creation_time: epoch_to_rfc3339(now),
                energy_source_preference: None,
                energy_type: None,
                updated_at: None,
                reject_reason: None,
                preferred_trading_partner: None,
                preferred_energy_rate: None,
            };
            publish_rejection(&ewds, submission, reason).await;
        }
    }
    Ok(input_orders)
}
