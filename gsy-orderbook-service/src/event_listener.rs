use crate::db::DbRef;
use crate::schema::OrderStatus;
use anyhow::{Error, Result};
use mongodb::bson;
use subxt::{
    SubstrateConfig,
    OnlineClient,
};
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

pub async fn init_event_listener(db: DbRef, node_url: String) -> Result<(), Error> {

    let api = OnlineClient::<SubstrateConfig>::from_url(format!("ws://{}", node_url)).await?;

    let mut gsy_node_blocks = api.blocks().subscribe_all().await?;

    while let Some(block) = gsy_node_blocks.next().await {
        let block = block?;

        // Ask for the events for this block.
        let events = block.events().await?;

        let block_hash = block.hash();
        // let events = events?;
        // let block_hash = events.block_hash;
        // let event = events.event;
        info!("Events at block {:?}:", block_hash);
        for event in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
            if let Ok(order_executed) = &event {
                info!("Order Executed: {:?}", order_executed);

                let id = &bson::to_bson(&order_executed.0.offer_hash).unwrap();
                match db
                    .get_ref()
                    .orders()
                    .update_order_status_by_id(id, OrderStatus::Executed)
                    .await
                {
                    Ok(result) => info!("Update result: {:?}", result),
                    Err(e) => {
                        tracing::error!("Failed to execute update: {:?}", e);
                    }
                }

                let id = &bson::to_bson(&order_executed.0.bid_hash).unwrap();
                match db
                    .get_ref()
                    .orders()
                    .update_order_status_by_id(id, OrderStatus::Executed)
                    .await
                {
                    Ok(result) => info!("Update result: {:?}", result),
                    Err(e) => {
                        tracing::error!("Failed to execute update: {:?}", e);
                    }
                }
            }
        }

        for event in events.find::<gsy_node::orderbook_registry::events::OrderDeleted>() {
            if let Ok(order_deleted) = &event {
                info!("Hash of the removed order: {:?}", order_deleted.1);
                let id = &bson::to_bson(&order_deleted.1).unwrap();
                match db
                    .get_ref()
                    .orders()
                    .update_order_status_by_id(id, OrderStatus::Deleted)
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
