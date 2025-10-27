use crate::primitives::penalty_calculator::Penalty;
use anyhow::{anyhow, Error, Result};
use codec::{Decode, Encode};
use std::str::FromStr;
use subxt::{
	utils::{AccountId32, H256},
	OnlineClient, SubstrateConfig,
};
use subxt_signer::sr25519::dev;
use gsy_offchain_primitives::utils::string_to_h256;
use tracing::info;

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

pub async fn submit_penalties(node_url: &str, penalties: Vec<Penalty>) -> Result<(), Error> {
	if penalties.is_empty() {
		info!("No penalties to submit.");
		return Ok(());
	}

	type NodeTradesPenalties =
		gsy_node::runtime_types::gsy_primitives::trades::TradesPenalties<AccountId32, H256>;

	let node_penalties: Vec<NodeTradesPenalties> = penalties
		.iter()
		.filter_map(|p| {
			let account = AccountId32::from_str(&p.penalized_account).ok()?;
			let market_uuid = string_to_h256(p.market_id.clone());
			let trade_uuid = string_to_h256(p.trade_uuid.clone());

			Some(NodeTradesPenalties {
				penalized_account: account,
				market_uuid,
				trade_uuid,
				penalty_energy: p.penalty_cost,
			})
		})
		.collect();

	info!("Sending {} penalties to gsy-node.", node_penalties.len());
	let penalty_extrinsic = gsy_node::tx().trades_settlement().submit_penalties(node_penalties);

	let signer = dev::alice();

	let api = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url).await?;
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
