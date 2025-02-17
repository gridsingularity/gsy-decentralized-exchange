use anyhow::{Error, Result};
use codec::{Decode, Encode};
use subxt::{OnlineClient, SubstrateConfig, utils::AccountId32};
use subxt_signer::sr25519::dev;
use tracing::{error, info};
use crate::primitives::penalty_calculator::Penalty;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node { }

/// Submits the penalty extrinsics to the node.
///
/// # Arguments
///
/// * `node_url` - The URL of the substrate node.
/// * `penalties` - A slice of Penalty structs to submit.
///
/// # Returns
///
/// A Result indicating success or failure.
pub async fn submit_penalties(
    node_url: &str,
    penalties: Vec<Penalty>,
) -> Result<(), Error> {
    if penalties.is_empty() {
        info!("No penalties to submit.");
        return Ok(());
    }
    
    // Connect to the node using subxt.
    let api = OnlineClient::<SubstrateConfig>::from_url(node_url).await?;
    
    // Convert our internal Penalty vector into the runtime type.
    // The node expects a vector of TradesPenalties<AccountId, Hash> with:
    // - penalized_account: AccountId32
    // - market_uuid: u32
    // - trade_uuid: Hash (we'll use String here for simplicity)
    // - penalty_energy: u64
    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub struct TradesPenalties {
        pub penalized_account: AccountId32,
        pub market_uuid: u32,
        pub trade_uuid: String,
        pub penalty_energy: u64,
    }
    
    // Convert each internal Penalty into the node type.
    let node_penalties: Vec<TradesPenalties> = penalties.iter().filter_map(|p| {
        // Convert penalized_account (String) to AccountId32.
        let account = AccountId32::from_string(&p.penalized_account).ok()?;
        // Convert market_uuid from String to u32.
        let market_uuid = p.market_uuid.parse::<u32>().ok()?;
        Some(TradesPenalties {
            penalized_account: account,
            market_uuid,
            trade_uuid: p.trade_uuid.clone(),
            penalty_energy: p.penalty_cost,
        })
    }).collect();
    
    // Build the extrinsic call.
    // Here we assume that the runtime exposes a pallet "penalty_submission"
    // with a call "submit_penalties" that takes a Vec<TradesPenalties>.
    let penalty_extrinsic = gsy_node::tx().trades_settlement().submit_penalties(node_penalties);
    
    // Use a development signer (for example, dev::alice()).
    let signer = dev::alice();
    
    // Sign and submit the extrinsic and wait for finalization.
    let tx_progress = api
        .tx()
        .sign_and_submit_then_watch_default(&penalty_extrinsic, &signer)
        .await?
        .wait_for_finalized_success()
        .await?;
    
    // Optionally, look for an event that confirms submission.
    let event = tx_progress.find_first::<gsy_node::trades_settlement::events::PenaltiesSubmitted>()?;
    if let Some(e) = event {
        info!("Penalty submission successful: {:?}", e);
    } else {
        info!("Penalty submission extrinsic finalized but event not found");
    }
    
    Ok(())
}
