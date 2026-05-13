use crate::db::DbRef;
use gsy_offchain_primitives::db_api_schema::orders::OrderStatus;
use chrono::{Local, Utc};
use tokio_schedule::{every, Job};

pub async fn start_scheduler(db: DbRef, scheduler_interval: u32) {
    let every_ten_minute = every(scheduler_interval)
        .seconds()
        .in_timezone(&Utc)
        .perform(|| async {
            match db
                .get_ref()
                .orders()
                .update_expired_orders(
                    Local::now().timestamp() as u64,
                    OrderStatus::Expired
                ).await
            {
                Ok(result) => tracing::info!("Update result: {:?}", result),
                Err(e) => {
                    tracing::error!("Failed to execute update: {:?}", e);
                }
            }
        });
    every_ten_minute.await;
}
