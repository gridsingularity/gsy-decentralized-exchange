use crate::db::DbRef;
use chrono::Utc;
use gsy_offchain_primitives::db_api_schema::orders::OrderStatus;
use tokio_schedule::{every, Job};

pub async fn start_scheduler(db: DbRef, scheduler_interval: u32) {
    let every_interval = every(scheduler_interval)
        .seconds()
        .in_timezone(&Utc)
        .perform(|| async {
            let now = Utc::now().timestamp() as u64;
            match db
                .get_ref()
                .orders()
                .update_expired_orders(now, OrderStatus::Expired)
                .await
            {
                Ok(result) => tracing::info!("Update result: {:?}", result),
                Err(e) => {
                    tracing::error!("Failed to execute update: {:?}", e);
                }
            }
        });
    every_interval.await;
}
