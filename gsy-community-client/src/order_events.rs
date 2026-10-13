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
use tokio::time::{sleep, timeout, Duration};
use tracing::{info, warn};
use uuid::Uuid;

const CONSUMER_CLIENT_ID_ENV: &str = "EWDS_COMMUNITY_CLIENT_ID";
const CONSUMER_CLIENT_ID_DEFAULT: &str = "gsycommunityclient";
const EWDS_TIMEOUT_MS: u64 = 60_000;
const PLACE_ORDER_ATTEMPTS: u32 = 3;
const PLACE_ORDER_RETRY_DELAY_MS: u64 = 1_000;
const NODE_HEALTH_CHECK_INTERVAL_MS: u64 = 30_000;
const NODE_HEALTH_CHECK_TIMEOUT_MS: u64 = 10_000;

/// Polls `order.event` messages from EWDS and sends their orders to the `OrderRegistry`. Runs until the
/// task is dropped, and only returns early if the node or the configuration is unusable, or the
/// connection to the node is lost for good.
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
    tokio::select! {
        _ = ewds_client.run_event_worker(EwdsEventType::Order, |envelope| {
            handler.handle(envelope)
        }) => Ok(()),
        error = watch_node(&handler.order_registry) => Err(error),
    }
}

/// Checks the connection to the node periodically and returns once a check fails. The websocket
/// provider does not report when it gives up reconnecting, and never recovers from it; every
/// request fails from then on, so the subscriber has to stop and be restarted.
async fn watch_node(order_registry: &OrderRegistryClient) -> Error {
    loop {
        sleep(Duration::from_millis(NODE_HEALTH_CHECK_INTERVAL_MS)).await;
        if let Err(error) = order_registry.check_connection().await {
            return error.context("Lost the connection to the EVM node");
        }
    }
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

    /// Sends the orders of an `order.event` to the `OrderRegistry`.
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

    /// Fails if the node does not answer in time, e.g. because the provider gave up reconnecting.
    pub async fn check_connection(&self) -> Result<()> {
        timeout(
            Duration::from_millis(NODE_HEALTH_CHECK_TIMEOUT_MS),
            self.contract.client().get_block_number(),
        )
        .await
        .context("The EVM node did not answer in time")??;
        Ok(())
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
pub fn validate_order(order: DbOrderSchema) -> Result<DbOrderSchema> {
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
