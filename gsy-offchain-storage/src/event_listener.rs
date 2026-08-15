use crate::db::DbRef;
use anyhow::{Error, Result};
use gsy_offchain_primitives::db_api_schema::orders::OrderStatus;
use gsy_offchain_primitives::db_api_schema::trades::TradeStatus;
use gsy_offchain_primitives::utils::h256_to_string;
use mongodb::bson;
use subxt::utils::H256;
use subxt::{OnlineClient, SubstrateConfig};
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

pub async fn init_event_listener(db: DbRef, node_url: String) -> Result<(), Error> {
    let api =
        OnlineClient::<SubstrateConfig>::from_insecure_url(format!("ws://{}", node_url)).await?;

    let mut gsy_node_blocks = api.blocks().subscribe_all().await?;

    while let Some(block) = gsy_node_blocks.next().await {
        let block = block?;

        // Ask for the events for this block.
        let events = block.events().await?;

        let block_hash = block.hash();
        info!("Events at block {:?}:", block_hash);
        for event in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
            if let Ok(order_executed) = &event {
                let trade = &order_executed.0;
                info!("Order Executed: {:?}", trade);

                // Mark the matched offer and bid as executed in the off-chain orderbook so the
                // matching engine stops offering them. Any residual order left by a partial match
                // is synced to the orderbook by the node's offchain worker, so the listener only
                // needs to handle the status transition here.
                mark_order_executed(&db, trade.offer_hash).await;
                mark_order_executed(&db, trade.bid_hash).await;
            }
        }

        for event in events.find::<gsy_node::orderbook_registry::events::OrderDeleted>() {
            if let Ok(order_deleted) = &event {
                info!("Hash of the removed order: {:?}", order_deleted.1);
                let id = bson::Bson::String(h256_to_string(order_deleted.1));
                match db
                    .get_ref()
                    .orders()
                    .update_order_status_by_id(&id, OrderStatus::Deleted)
                    .await
                {
                    Ok(result) => info!("Update result: {:?}", result),
                    Err(e) => {
                        tracing::error!("Failed to execute update: {:?}", e);
                    }
                }
            }
        }

        // A single batch can carry more than one penalty for the same trade_uuid (independent
        // buyer/seller passes), so mark_trade_penalized may run twice per trade per block; the
        // update is idempotent, so this is harmless.
        for event in events.find::<gsy_node::trades_settlement::events::PenaltiesSubmitted>() {
            if let Ok(penalties_submitted) = &event {
                mark_trade_penalized(&db, penalties_submitted.0.trade_uuid).await;
            }
        }

        // subscribe_all() yields non-finalized blocks, so a re-org can replay this event; the
        // status update is idempotent, so replay is harmless.
        for event in events.find::<gsy_node::trades_settlement::events::TradeExecuted>() {
            if let Ok(trade_executed) = &event {
                mark_trade_executed(&db, trade_executed.0).await;
            }
        }
    }

    Ok(())
}

/// Mark an order identified by its on-chain hash as executed in the off-chain orderbook.
async fn mark_order_executed(db: &DbRef, order_hash: H256) {
    let id = bson::Bson::String(h256_to_string(order_hash));
    match db
        .get_ref()
        .orders()
        .update_order_status_by_id(&id, OrderStatus::Executed)
        .await
    {
        Ok(result) => info!("Marked order {:?} as executed: {:?}", id, result),
        Err(e) => {
            tracing::error!("Failed to mark order as executed: {:?}", e);
        }
    }
}

/// Set the status of the trade identified by its on-chain `trade_uuid`.
///
/// The uuids in these events originate from a `GET /trades` read that the execution engine
/// performs against this very service before signing the extrinsic, so the trade row provably
/// exists by the time the event is emitted. A `matched_count == 0` therefore indicates real data
/// loss rather than a benign race, which is why this warns instead of retrying.
async fn set_trade_status(db: &DbRef, trade_uuid: H256, status: TradeStatus) {
    let trade_uuid = h256_to_string(trade_uuid);
    match db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, status.clone())
        .await
    {
        Ok(result) if result.matched_count == 0 => {
            tracing::warn!(
                "No trade found for trade_uuid {} when marking {:?}",
                trade_uuid,
                status
            );
        }
        Ok(result) => info!("Marked trade {} as {:?}: {:?}", trade_uuid, status, result),
        Err(e) => {
            tracing::error!("Failed to mark trade as {:?}: {:?}", status, e);
        }
    }
}

/// Mark the trade identified by its on-chain `trade_uuid` as executed (evaluated, no penalty).
async fn mark_trade_executed(db: &DbRef, trade_uuid: H256) {
    set_trade_status(db, trade_uuid, TradeStatus::Executed).await;
}

/// Mark the trade identified by its on-chain `trade_uuid` as penalized (evaluated, penalty
/// submitted on-chain).
async fn mark_trade_penalized(db: &DbRef, trade_uuid: H256) {
    set_trade_status(db, trade_uuid, TradeStatus::Penalized).await;
}
