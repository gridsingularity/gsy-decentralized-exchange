use crate::db::DbRef;
use anyhow::{Error, Result};

pub async fn init_event_listener(_db: DbRef, _node_url: String) -> Result<(), Error> {
    // TODO: Integrate with Ethereum Event Listener
    Ok(())
}
