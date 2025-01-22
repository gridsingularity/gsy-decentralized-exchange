use anyhow::Result;
use tracing::info;
use crate::primitives::penalty_calculator::Penalty;

pub async fn submit_penalties(
    node_url: &str,
    penalties: &[Penalty],
) -> Result<()> {
    if penalties.is_empty() {
        info!("No penalties to submit.");
        return Ok(());
    }
    // TODO: actual extrinsic logic
    Ok(())
}
