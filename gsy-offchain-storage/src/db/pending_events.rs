use anyhow::{ensure, Result};
use gsy_ethers_listener::{ChainEvent, GsyContractsEvents};
use mongodb::{bson::doc, Collection};
use primitives::utils::bytes16_to_uuid_string;
use serde::{Deserialize, Serialize};

pub(crate) const COLLECTION: &str = "evm_pending_events";

/// Only missing identities/projections are deferrable. Database and invariant errors
/// still fail the block, retaining the listener's existing retry behavior.
#[derive(Debug)]
pub(crate) struct UnresolvedEvent(pub String);

impl std::fmt::Display for UnresolvedEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unresolved chain event: {}", self.0)
    }
}
impl std::error::Error for UnresolvedEvent {}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PendingEvent {
    #[serde(rename = "_id")]
    pub id: String,
    pub stream_id: String,
    pub block_number: i64,
    pub log_index: i64,
    pub dependencies: Vec<String>,
    pub market_ids: Vec<String>,
    pub event: ChainEvent,
}

impl PendingEvent {
    pub fn new(event: ChainEvent) -> Result<Self> {
        let meta = event.metadata()?;
        let mut dependencies = Vec::new();
        let mut market_ids = Vec::new();
        let mut order = |id| dependencies.push(format!("order:{}", bytes16_to_uuid_string(id)));
        let market_id = match event.decode()? {
            GsyContractsEvents::OrderPlacedFilter(e) => {
                order(e.order_id);
                Some(e.market_id)
            }
            GsyContractsEvents::OrderCancelledFilter(e) => {
                order(e.order_id);
                None
            }
            GsyContractsEvents::TradeMatchedFilter(e) => {
                order(e.bid_id);
                order(e.offer_id);
                for id in [e.residual_bid_id, e.residual_offer_id] {
                    if id != [0; 16] {
                        order(id);
                    }
                }
                dependencies.push(format!("trade:{}", bytes16_to_uuid_string(e.trade_id)));
                Some(e.market_id)
            }
            GsyContractsEvents::DeliveryVerifiedFilter(e) => {
                order(e.bid_id);
                order(e.offer_id);
                dependencies.push(format!("trade:{}", bytes16_to_uuid_string(e.trade_id)));
                None
            }
            GsyContractsEvents::PenaltyRecordedFilter(e) => {
                dependencies.push(format!("trade:{}", bytes16_to_uuid_string(e.trade_id)));
                Some(e.market_id)
            }
            GsyContractsEvents::NewMarketCreatedFilter(e) => Some(e.market_id),
            GsyContractsEvents::MarketClearingFilter(e) => Some(e.market_id),
            _ => None,
        };
        if let Some(id) = market_id {
            let id = bytes16_to_uuid_string(id);
            dependencies.push(format!("market:{id}"));
            market_ids.push(id);
        }
        Ok(Self {
            id: format!(
                "{}:{:?}:{}",
                event.stream_id, meta.block_hash, meta.log_index
            ),
            stream_id: event.stream_id.clone(),
            block_number: i64::try_from(meta.block_number.as_u64())?,
            log_index: i64::try_from(meta.log_index.as_u64())?,
            dependencies,
            market_ids,
            event,
        })
    }
}

/// Never advertise a known-incomplete projection as a complete book/trade set.
/// Unknown market scope conservatively blocks all such reads, not chain indexing.
pub(crate) async fn ensure_ready<T: Send + Sync>(
    collection: &Collection<T>,
    market_id: Option<&str>,
) -> Result<()> {
    let database = collection.client().database(&collection.namespace().db);
    let filter = match market_id {
        Some(id) => doc! {"$or": [{"market_ids": id}, {"market_ids": {"$size": 0}}]},
        None => doc! {},
    };
    ensure!(
        database
            .collection::<PendingEvent>(COLLECTION)
            .find_one(filter)
            .await?
            .is_none(),
        "Chain projection is incomplete: unresolved events are pending"
    );
    Ok(())
}
