use crate::db::DbRef;
use crate::schema::OrderStatus;
use anyhow::{Error, Result};
use futures::StreamExt;
use mongodb::bson;
use subxt::{ClientBuilder, DefaultConfig, SubstrateExtrinsicParams};
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

pub async fn init_event_listener(db: DbRef, node_url: String) -> Result<(), Error> {
    let api = ClientBuilder::new()
        .set_url(format!("ws://{}", node_url))
        .build()
        .await?
        .to_runtime_api::<gsy_node::RuntimeApi<DefaultConfig, SubstrateExtrinsicParams<DefaultConfig>>>();

    let mut gsy_node_events = api.events().subscribe().await?.filter_events::<(
        gsy_node::orderbook_registry::events::OrderExecuted,
        gsy_node::orderbook_registry::events::OrderDeleted,
    )>();

    while let Some(events) = gsy_node_events.next().await {
        let events = events?;
        let block_hash = events.block_hash;
        let event = events.event;
        info!("Events at block {:?}:", block_hash);

        if let (Some(order_executed), _) = &event {
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
        if let (_, Some(order_deleted)) = &event {
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

    Ok(())
}
