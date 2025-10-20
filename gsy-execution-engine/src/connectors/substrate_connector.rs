use crate::primitives::penalty_calculator::Penalty;
use anyhow::{anyhow, Error, Result};
use codec::{Decode, Encode};
use std::str::FromStr;
use subxt::{
	utils::{AccountId32, H256},
	OnlineClient, SubstrateConfig,
};
use subxt_signer::sr25519::dev;
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

pub async fn submit_penalties(node_url: &str, penalties: Vec<Penalty>) -> Result<(), Error> {
	if penalties.is_empty() {
		info!("No penalties to submit.");
		return Ok(());
	}

	let api = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;

	type NodeTradesPenalties =
		gsy_node::runtime_types::gsy_primitives::trades::TradesPenalties<AccountId32, H256>;

	let node_penalties: Vec<NodeTradesPenalties> = penalties
		.iter()
		.filter_map(|p| {
			let account = AccountId32::from_str(&p.penalized_account).ok()?;
			let market_uuid = p.market_id.parse::<u32>().ok()?;
			let trade_uuid = H256::from_str(&p.trade_uuid).ok()?;

			Some(NodeTradesPenalties {
				penalized_account: account,
				market_uuid,
				trade_uuid,
				penalty_energy: p.penalty_cost,
			})
		})
		.collect();

	let penalty_extrinsic = gsy_node::tx().trades_settlement().submit_penalties(node_penalties);

	let signer = dev::alice();

	let tx_progress = api
		.tx()
		.sign_and_submit_then_watch_default(&penalty_extrinsic, &signer)
		.await?
		.wait_for_finalized_success()
		.await?;

	let event =
		tx_progress.find_first::<gsy_node::trades_settlement::events::PenaltiesSubmitted>()?;
	if let Some(e) = event {
		info!("Penalty submission successful: {:?}", e);
	} else {
		info!("Penalty submission extrinsic finalized but event not found");
	}

	Ok(())
}
