use crate::node_connector::orders::{
    EvmOrderParamsTuple, OrderRegistryContract, OrderRegistryContractErrors,
};
use anyhow::{anyhow, bail, Context, Error, Result};
use ethers::prelude::*;
use primitives::db_api_schema::orders::{
    order_metadata_to_contract, DbOrderSchema, OrderEnum, OrderStatus,
};
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsOrderDto};
use primitives::ewds::{env_var, invalid_event, parse_batch, EwdsClient, EwdsEventType};
use primitives::offchain_storage::{resolve_order_partner_ids, OffchainStorageClient};
use primitives::utils::{parse_uuid_or_hex_bytes16, NODE_FLOAT_SCALING_FACTOR};
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use uuid::Uuid;

const CONSUMER_CLIENT_ID_ENV: &str = "EWDS_COMMUNITY_CLIENT_ID";
const CONSUMER_CLIENT_ID_DEFAULT: &str = "gsycommunityclient";
const EWDS_TIMEOUT_MS: u64 = 60_000;
const PLACE_ORDER_ATTEMPTS: u32 = 3;
const PLACE_ORDER_RETRY_DELAY_MS: u64 = 1_000;

/// Polls `order.submitted` events from EWDS and sends their orders to the `OrderRegistry`. Runs until the
/// task is dropped, and only returns early if the node or the configuration is unusable.
pub async fn start_order_event_subscriber() -> Result<()> {
    let evm_node_url = env_var("EVM_NODE_URL").unwrap_or_else(|| "ws://anvil:8545".to_string());
    let order_registry_address =
        env_var("ORDER_REGISTRY_ADDRESS").context("ORDER_REGISTRY_ADDRESS is not set")?;
    let private_key = env_var("COMMUNITY_CLIENT_PRIVATE_KEY")
        .context("COMMUNITY_CLIENT_PRIVATE_KEY is not set")?;

    let order_registry = OrderRegistryClient::connect(
        evm_node_url.as_str(),
        order_registry_address.as_str(),
        private_key.as_str(),
    )
    .await
    .with_context(|| format!("Failed to connect to the EVM node at {}", evm_node_url))?;
    let handler = OrderEventHandler::new(
        OffchainStorageClient::from_env(CONSUMER_CLIENT_ID_ENV, CONSUMER_CLIENT_ID_DEFAULT),
        order_registry,
    );
    let ewds_client = EwdsClient::from_env(
        CONSUMER_CLIENT_ID_ENV,
        CONSUMER_CLIENT_ID_DEFAULT,
        EWDS_TIMEOUT_MS,
    );

    info!(
        "Starting EWDS order event subscriber (order_registry={})",
        order_registry_address
    );
    ewds_client
        .run_event_subscriber(&[EwdsEventType::OrderSubmitted], |envelope| {
            handler.handle(envelope)
        })
        .await;
    Ok(())
}

/// What happened to one order of an event.
enum Placement {
    Sent(TxHash),
    AlreadyPlaced,
    Failed(PlaceOrderError),
}

pub struct OrderEventHandler {
    id_service: OffchainStorageClient,
    order_registry: OrderRegistryClient,
}

impl OrderEventHandler {
    pub fn new(id_service: OffchainStorageClient, order_registry: OrderRegistryClient) -> Self {
        Self {
            id_service,
            order_registry,
        }
    }

    /// Sends the orders of an `order.submitted` event to the `OrderRegistry`.
    ///
    /// The event is rejected before anything is sent if one of its orders is invalid or its IDs
    /// cannot be resolved. After that every order is sent on its own: one the contract rejects
    /// is logged and the others are still sent, because sent orders cannot be rolled back. If an
    /// order could not be sent because the node was unreachable, the event fails after the other
    /// orders were sent, so the event worker retries it; orders already placed are skipped then.
    /// The handler does not wait for the transactions to be mined.
    pub async fn handle(&self, envelope: EwdsEventEnvelope<Value>) -> Result<()> {
        let event_id = envelope.event_id;
        let orders = parse_batch(envelope.data, |order: EwdsOrderDto| {
            DbOrderSchema::try_from(order).and_then(validate_order)
        })?;

        let mut params = Vec::with_capacity(orders.len());
        for (index, order) in orders.into_iter().enumerate() {
            let order_id = order.order_id.clone();
            let order = self
                .resolve_ids(order)
                .await
                .and_then(|order| order_params(&order).map_err(invalid_event))
                .with_context(|| format!("invalid order {} at index {}", order_id, index))?;
            params.push((order_id, order));
        }

        let total = params.len();
        let mut sent = 0;
        let mut unsent = 0;
        for (order_id, order) in params {
            match self.place(order).await {
                Placement::Sent(tx_hash) => {
                    sent += 1;
                    info!(
                        "Sent order {} of EWDS event {} (tx={:?})",
                        order_id, event_id, tx_hash
                    );
                }
                Placement::AlreadyPlaced => {
                    sent += 1;
                    info!(
                        "Order {} of EWDS event {} is already placed; skipping it",
                        order_id, event_id
                    );
                }
                Placement::Failed(error) => {
                    if matches!(error, PlaceOrderError::Transport(_)) {
                        unsent += 1;
                    }
                    warn!(
                        "Order {} of EWDS event {} was not sent: {}",
                        order_id, event_id, error
                    );
                }
            }
        }
        info!(
            "EWDS event {}: {} of {} orders sent or already placed",
            event_id, sent, total
        );
        if unsent > 0 {
            bail!(
                "{} of {} orders of EWDS event {} could not be sent to the node",
                unsent,
                total,
                event_id
            );
        }
        Ok(())
    }

    /// Replaces the off-chain actor and partner IDs with their on-chain IDs.
    async fn resolve_ids(&self, mut order: DbOrderSchema) -> Result<DbOrderSchema> {
        order.created_by = self
            .id_service
            .fetch_onchain_id(&order.created_by)
            .await
            .with_context(|| format!("createdBy '{}' could not be resolved", order.created_by))?;
        order.area_uuid = order.created_by.clone();
        resolve_order_partner_ids(&mut order.requirements, &self.id_service)
            .await
            .context("preferredTradingPartner could not be resolved")?;
        Ok(order)
    }

    /// Sends one order, retrying transport errors. An order that is already on-chain, or that
    /// the contract reports as existing, is not sent again.
    async fn place(&self, params: EvmOrderParamsTuple) -> Placement {
        let mut attempt = 1;
        loop {
            let result = match self.order_registry.is_placed(params.0).await {
                Ok(true) => return Placement::AlreadyPlaced,
                Ok(false) => self.order_registry.place_order(params).await,
                Err(error) => Err(error),
            };
            match result {
                Ok(tx_hash) => return Placement::Sent(tx_hash),
                Err(PlaceOrderError::Rejected(reason)) if reason == ORDER_ALREADY_EXISTS => {
                    return Placement::AlreadyPlaced
                }
                Err(PlaceOrderError::Transport(error)) if attempt < PLACE_ORDER_ATTEMPTS => {
                    warn!(
                        "placeOrder attempt {} failed, retrying: {:#}",
                        attempt, error
                    );
                    sleep(Duration::from_millis(
                        PLACE_ORDER_RETRY_DELAY_MS * u64::from(attempt),
                    ))
                    .await;
                    attempt += 1;
                }
                Err(error) => return Placement::Failed(error),
            }
        }
    }
}

const WS_RECONNECTS: usize = 5;

type OrderRegistrySigner = SignerMiddleware<Provider<Ws>, LocalWallet>;

/// Why an order was not placed.
#[derive(Debug)]
pub enum PlaceOrderError {
    /// The contract rejected the order when the node estimated gas, so nothing was sent.
    /// Sending it again would fail the same way.
    Rejected(String),
    /// The node could not be reached or did not accept the transaction. Sending it again may
    /// work.
    Transport(Error),
}

impl std::fmt::Display for PlaceOrderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(reason) => write!(formatter, "rejected by the contract: {}", reason),
            Self::Transport(error) => write!(formatter, "{:#}", error),
        }
    }
}

/// Places orders on the `OrderRegistry`, signed with the community client's wallet.
pub struct OrderRegistryClient {
    contract: OrderRegistryContract<OrderRegistrySigner>,
}

impl OrderRegistryClient {
    pub async fn connect(
        evm_node_url: &str,
        order_registry_address: &str,
        private_key: &str,
    ) -> Result<Self> {
        let order_registry_address = Address::from_str(order_registry_address)
            .map_err(|e| anyhow!("Invalid order registry address: {}", e))?;
        if order_registry_address.is_zero() {
            warn!(
                "ORDER_REGISTRY_ADDRESS is zero; placeOrder transactions will fail until configured."
            );
        }

        let provider = Provider::<Ws>::connect_with_reconnects(evm_node_url, WS_RECONNECTS).await?;
        let chain_id = provider.get_chainid().await?.as_u64();
        let wallet = private_key
            .parse::<LocalWallet>()
            .map_err(|e| anyhow!("Invalid community client private key: {}", e))?
            .with_chain_id(chain_id);
        let client = Arc::new(SignerMiddleware::new(provider, wallet));

        Ok(Self {
            contract: OrderRegistryContract::new(order_registry_address, client),
        })
    }

    /// Whether an order with this ID was ever placed, whatever its status is now.
    pub async fn is_placed(&self, order_id: [u8; 16]) -> Result<bool, PlaceOrderError> {
        let status = self
            .contract
            .get_status(order_id)
            .call()
            .await
            .map_err(|e| PlaceOrderError::Transport(e.into()))?;
        Ok(status != ORDER_STATUS_NONE)
    }

    /// Sends `placeOrder` and returns as soon as the node accepted the transaction, without
    /// waiting for it to be mined. A revert is only seen when the node estimates gas before
    /// sending; one that happens in the mined transaction goes unnoticed.
    pub async fn place_order(
        &self,
        params: EvmOrderParamsTuple,
    ) -> Result<TxHash, PlaceOrderError> {
        // Earlier transactions may still be pending, so the nonce has to count them too.
        let client = self.contract.client();
        let nonce = client
            .get_transaction_count(client.address(), Some(BlockNumber::Pending.into()))
            .await
            .map_err(|e| PlaceOrderError::Transport(e.into()))?;
        let call = self.contract.place_order(params).nonce(nonce);
        let pending_tx = call.send().await.map_err(classify_contract_error)?;
        Ok(pending_tx.tx_hash())
    }
}

/// `OrderRegistry.OrderStatus.None`: no order with this ID exists.
const ORDER_STATUS_NONE: u8 = 0;

fn classify_contract_error(error: ContractError<OrderRegistrySigner>) -> PlaceOrderError {
    if !error.is_revert() {
        return PlaceOrderError::Transport(error.into());
    }
    let reason = match error.decode_contract_revert::<OrderRegistryContractErrors>() {
        Some(OrderRegistryContractErrors::MarketClosed(_)) => "MarketClosed".to_string(),
        Some(OrderRegistryContractErrors::Unauthorized(_)) => "Unauthorized".to_string(),
        Some(OrderRegistryContractErrors::InvalidOrderParams(_)) => {
            "InvalidOrderParams".to_string()
        }
        Some(OrderRegistryContractErrors::OrderNotOpen(_)) => "OrderNotOpen".to_string(),
        Some(OrderRegistryContractErrors::OrderAlreadyExists(_)) => {
            ORDER_ALREADY_EXISTS.to_string()
        }
        Some(OrderRegistryContractErrors::RevertString(message)) => message,
        None => format!(
            "undecoded revert data {}",
            error
                .as_revert()
                .map(|data| data.to_string())
                .unwrap_or_default()
        ),
    };
    PlaceOrderError::Rejected(reason)
}

/// The reason [`PlaceOrderError::Rejected`] carries when the order ID is already taken.
pub const ORDER_ALREADY_EXISTS: &str = "OrderAlreadyExists";

/// Contract parameters for an order whose order, actor, market and partner IDs are already
/// on-chain IDs.
pub fn order_params(order: &DbOrderSchema) -> Result<EvmOrderParamsTuple> {
    let onchain_id = |name: &str, value: &str| {
        parse_uuid_or_hex_bytes16(value)
            .filter(|bytes| *bytes != [0; 16])
            .ok_or_else(|| anyhow!("{} '{}' is not a 16-byte UUID or hex ID", name, value))
    };
    let metadata =
        order_metadata_to_contract(order.requirements.as_ref(), order.attributes.as_ref())?;

    Ok((
        onchain_id("orderId", &order.order_id)?,
        onchain_id("createdBy", &order.created_by)?,
        onchain_id("marketId", &order.market_id)?,
        order.time_slot,
        order.creation_time,
        (order.energy_kWh * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        (order.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
        metadata.energy_source_preference,
        metadata.energy_type,
        matches!(order.order_type, OrderEnum::Bid),
        metadata.preferred_trading_partner,
        metadata.preferred_energy_rate,
    ))
}

/// The field rules an order from EWDS has to meet before it is placed.
fn validate_order(order: DbOrderSchema) -> Result<DbOrderSchema> {
    Uuid::parse_str(&order.order_id)
        .map_err(|e| anyhow!("orderId '{}' is not a UUID: {}", order.order_id, e))?;
    if order.status != OrderStatus::Submitted {
        bail!("orderStatus must be 'submitted', got {:?}", order.status);
    }
    if !(order.energy_kWh > 0.0 && order.energy_kWh.is_finite()) {
        bail!("quantity must be greater than 0, got {}", order.energy_kWh);
    }
    if !(order.energy_rate >= 0.0 && order.energy_rate.is_finite()) {
        bail!("priceLimit must be 0 or more, got {}", order.energy_rate);
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f".to_string(),
            status: OrderStatus::Submitted,
            order_type: OrderEnum::Bid,
            area_uuid: "owner-1".to_string(),
            market_id: format!("0x{}", "11".repeat(16)),
            time_slot: 1_790_762_400,
            creation_time: 1_790_761_212,
            energy_kWh: 1.5,
            energy_rate: 0.3,
            created_by: "owner-1".to_string(),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn accepts_a_valid_order() {
        assert_eq!(validate_order(order()).unwrap(), order());
    }

    #[test]
    fn accepts_a_price_limit_of_zero() {
        assert!(validate_order(DbOrderSchema {
            energy_rate: 0.0,
            ..order()
        })
        .is_ok());
    }

    #[test]
    fn rejects_invalid_orders() {
        for (invalid, expected) in [
            (
                DbOrderSchema {
                    order_id: "order-1".to_string(),
                    ..order()
                },
                "is not a UUID",
            ),
            (
                DbOrderSchema {
                    status: OrderStatus::Cancelled,
                    ..order()
                },
                "orderStatus",
            ),
            (
                DbOrderSchema {
                    energy_kWh: 0.0,
                    ..order()
                },
                "quantity",
            ),
            (
                DbOrderSchema {
                    energy_kWh: -1.0,
                    ..order()
                },
                "quantity",
            ),
            (
                DbOrderSchema {
                    energy_rate: -0.1,
                    ..order()
                },
                "priceLimit",
            ),
        ] {
            let error = validate_order(invalid).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}

#[cfg(test)]
mod order_params_tests {
    use super::*;
    use primitives::db_api_schema::orders::{DbAttributes, DbRequirements, EnergyType};
    use primitives::utils::bytes16_to_hex;

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f".to_string(),
            status: OrderStatus::Submitted,
            order_type: OrderEnum::Offer,
            area_uuid: format!("0x{}", "aa".repeat(16)),
            market_id: format!("0x{}", "11".repeat(16)),
            time_slot: 1_790_762_400,
            creation_time: 1_790_761_212,
            energy_kWh: 0.30005,
            energy_rate: 0.07,
            created_by: format!("0x{}", "aa".repeat(16)),
            requirements: None,
            attributes: Some(DbAttributes {
                energy_type: EnergyType::Battery,
            }),
        }
    }

    #[test]
    fn order_params_keeps_the_uuid_bytes_and_scales_energy_and_rate() {
        let params = order_params(&order()).unwrap();

        assert_eq!(
            bytes16_to_hex(params.0),
            "0x3f2c6d1e8a4b4c7d9e2f1a5b6c7d8e9f"
        );
        assert_eq!(params.1, [0xaa; 16]);
        assert_eq!(params.2, [0x11; 16]);
        assert_eq!((params.3, params.4), (1_790_762_400, 1_790_761_212));
        // Rounded, not truncated.
        assert_eq!((params.5, params.6), (3_001, 700));
        assert_eq!((params.7, params.8), (0, 5));
        assert!(!params.9);
        assert_eq!((params.10, params.11), ([0; 16], 0));
    }

    #[test]
    fn order_params_rejects_ids_that_are_not_on_chain_ids() {
        for (invalid, expected) in [
            (
                DbOrderSchema {
                    created_by: "owner-1".to_string(),
                    ..order()
                },
                "createdBy",
            ),
            (
                DbOrderSchema {
                    market_id: format!("0x{}", "00".repeat(16)),
                    ..order()
                },
                "marketId",
            ),
            (
                DbOrderSchema {
                    order_type: OrderEnum::Bid,
                    requirements: Some(DbRequirements {
                        trading_partner_id: Some("owner-2".to_string()),
                        energy_type: None,
                        preferred_energy_rate: None,
                    }),
                    attributes: None,
                    ..order()
                },
                "resolved through the ID service",
            ),
        ] {
            let error = order_params(&invalid).err().unwrap().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}
