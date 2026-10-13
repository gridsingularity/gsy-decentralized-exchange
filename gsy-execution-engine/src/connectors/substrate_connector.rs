use crate::primitives::penalty_calculator::Penalty;
use anyhow::{Error, Result};
use std::str::FromStr;
use subxt::{
	utils::{AccountId32, H256},
	OnlineClient, SubstrateConfig,
};
use subxt_signer::sr25519::dev;
use gsy_offchain_primitives::utils::string_to_h256;
use tracing::{info, warn};

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

/// Parses a `h256_to_string`-formatted hex string into an `H256`, returning `None` (rather
/// than panicking, unlike `string_to_h256`) if it is malformed.
fn parse_h256(hex_string: &str) -> Option<H256> {
	let hex_stripped = hex_string.strip_prefix("0x")?;
	// `string_to_h256` unwraps the hex decode, so the digits must be checked here too and not
	// just the length.
	if hex_stripped.len() != 64 || !hex_stripped.bytes().all(|b| b.is_ascii_hexdigit()) {
		return None;
	}
	Some(string_to_h256(hex_string.to_string()))
}

pub async fn submit_penalties(
	node_url: &str,
	penalties: Vec<Penalty>,
	evaluated_trade_uuids: Vec<String>,
) -> Result<(), Error> {
	if penalties.is_empty() && evaluated_trade_uuids.is_empty() {
		info!("No penalties and no evaluated trades to submit.");
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

	let node_uuids: Vec<H256> = evaluated_trade_uuids
		.iter()
		.filter_map(|uuid| match parse_h256(uuid) {
			Some(hash) => Some(hash),
			None => {
				warn!("Skipping malformed evaluated trade uuid: {}", uuid);
				None
			}
		})
		.collect();

	info!(
		"Sending {} penalties and {} evaluated trades to gsy-node.",
		node_penalties.len(),
		node_uuids.len()
	);
	let penalty_extrinsic = gsy_node::tx()
		.trades_settlement()
		.submit_penalties(node_penalties, node_uuids);

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

	let executed_count = tx_progress
		.find::<gsy_node::trades_settlement::events::TradeExecuted>()
		.filter_map(|e| e.ok())
		.count();
	info!("Marked {} trade(s) executed on-chain", executed_count);

	Ok(())
}
