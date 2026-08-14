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

/// Mark the trade identified by its on-chain `trade_uuid` as executed.
///
/// The trade row itself is written separately over HTTP by `post_trades`, with no ordering
/// guarantee relative to this event subscription, so the event can arrive first and find no
/// matching trade yet. Warn loudly in that case instead of leaving the trade silently stuck
/// on `Settled`.
async fn mark_trade_executed(db: &DbRef, trade_uuid: H256) {
    let trade_uuid = h256_to_string(trade_uuid);
    match db
        .get_ref()
        .trades()
        .update_trade_status_by_uuid(&trade_uuid, TradeStatus::Executed)
        .await
    {
        Ok(result) if result.matched_count == 0 => {
            tracing::warn!(
                "No trade found for trade_uuid {} when marking executed; the event may have \
                 arrived before the trade was inserted",
                trade_uuid
            );
        }
        Ok(result) => info!("Marked trade {} as executed: {:?}", trade_uuid, result),
        Err(e) => {
            tracing::error!("Failed to mark trade as executed: {:?}", e);
        }
    }
}
