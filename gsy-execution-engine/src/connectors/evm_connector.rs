use crate::primitives::penalty_calculator::{DeliveryVerification, Penalty};
use ::primitives::utils::parse_uuid_or_hex_bytes16;
use anyhow::{anyhow, Result};
use ethers::prelude::*;
use ethers::utils::keccak256;
use std::{str::FromStr, sync::Arc};
use tracing::{info, warn};

abigen!(
    SubmitPenaltiesContract,
    "src/connectors/abi/submit_penalties.json"
);

type EvmPenaltyTuple = ([u8; 16], [u8; 16], [u8; 16], u64);

/// Submit one atomic verdict per trade. The contract makes retries safe, including
/// retries after a successful transaction whose receipt was lost by this process.
pub async fn complete_delivery_verification(
    evm_node_url: &str,
    trade_settlement_address: &str,
    execution_engine_private_key: &str,
    verifications: Vec<DeliveryVerification>,
) -> Result<usize> {
    if verifications.is_empty() {
        return Ok(0);
    }
    let provider = Provider::<Ws>::connect(evm_node_url).await?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = execution_engine_private_key
        .parse::<LocalWallet>()?
        .with_chain_id(chain_id);
    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let contract =
        SubmitPenaltiesContract::new(trade_settlement_address.parse::<Address>()?, client);
    let processed = verifications.len();
    let mut pending = Vec::new();
    for verification in verifications {
        let id = parse_uuid_or_hex_bytes16(&verification.trade_uuid)
            .ok_or_else(|| anyhow!("Invalid trade ID: {}", verification.trade_uuid))?;
        let (bid_id, _, verified) = contract.settled_trades(id).call().await?;
        if bid_id == [0; 16] {
            return Err(anyhow!(
                "Trade {} is not registered for verification",
                verification.trade_uuid
            ));
        }
        if !verified {
            pending.push((
                id,
                verification.buyer_penalty_cost,
                verification.seller_penalty_cost,
            ));
        }
    }
    if !pending.is_empty() {
        let call = contract.complete_delivery_verification(pending);
        let tx = call.send().await?;
        let hash = tx.tx_hash();
        let receipt = tx.await?.ok_or_else(|| {
            anyhow!(
                "Verification transaction {:?} dropped without receipt",
                hash
            )
        })?;
        if receipt.status != Some(1u64.into()) {
            return Err(anyhow!("Verification transaction {:?} reverted", hash));
        }
        info!("Delivery verification successful. tx={:?}", hash);
    }
    Ok(processed)
}

fn to_evm_penalties(penalties: Vec<Penalty>) -> Vec<EvmPenaltyTuple> {
    penalties
        .into_iter()
        .filter_map(|penalty| {
            if penalty.penalty_cost == 0 {
                tracing::warn!(
                    "Skipping penalty for trade '{}' because penalty_cost is zero",
                    penalty.trade_uuid
                );
                return None;
            }
            let Some(penalized_account) = parse_uuid_or_hex_bytes16(&penalty.penalized_account)
            else {
                warn!(
                    "Skipping penalty for trade '{}': penalized_account '{}' is not a UUID",
                    penalty.trade_uuid, penalty.penalized_account
                );
                return None;
            };

            let Some(market_id) = parse_uuid_or_hex_bytes16(&penalty.market_id) else {
                warn!(
                    "Skipping penalty for trade '{}': market_id '{}' is not a UUID",
                    penalty.trade_uuid, penalty.market_id
                );
                return None;
            };

            let Some(trade_uuid) = parse_uuid_or_hex_bytes16(&penalty.trade_uuid) else {
                warn!(
                    "Skipping penalty for trade '{}': trade_uuid is not a UUID",
                    penalty.trade_uuid
                );
                return None;
            };

            Some((
                penalized_account,
                market_id,
                trade_uuid,
                penalty.penalty_cost,
            ))
        })
        .collect()
}

pub async fn submit_penalties(
    evm_node_url: &str,
    submit_penalties_contract_address: &str,
    execution_engine_private_key: &str,
    penalties: Vec<Penalty>,
) -> Result<usize> {
    if penalties.is_empty() {
        info!("No penalties to submit.");
        return Ok(0);
    }

    let submit_penalties_contract_address = Address::from_str(submit_penalties_contract_address)
        .map_err(|e| {
            anyhow!(
                "Invalid trade settlement address '{}': {}",
                submit_penalties_contract_address,
                e
            )
        })?;

    let evm_penalties = to_evm_penalties(penalties);
    if evm_penalties.is_empty() {
        info!("No valid penalties to submit after validation.");
        return Ok(0);
    }

    let provider = Provider::<Ws>::connect(evm_node_url).await?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = execution_engine_private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Invalid execution engine private key: {}", e))?
        .with_chain_id(chain_id);
    let signer_address = wallet.address();

    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let submit_penalties_contract =
        SubmitPenaltiesContract::new(submit_penalties_contract_address, client.clone());

    let execution_engine_role = keccak256("EXECUTION_ENGINE_ROLE");
    let has_role = submit_penalties_contract
        .has_role(execution_engine_role, signer_address)
        .call()
        .await?;
    if !has_role {
        warn!(
            "Signer {:?} does not currently have EXECUTION_ENGINE_ROLE in SubmitPenalties",
            signer_address
        );
    }

    let mut penalties_to_submit: Vec<EvmPenaltyTuple> = Vec::new();
    let mut skipped_existing = 0usize;
    for penalty in evm_penalties {
        let existing = submit_penalties_contract
            .penalty_energy_by_trade(penalty.2)
            .call()
            .await?;
        if existing.is_zero() {
            penalties_to_submit.push(penalty);
        } else {
            skipped_existing += 1;
        }
    }

    if penalties_to_submit.is_empty() {
        info!(
            "All computed penalties were already recorded on-chain (skipped {}).",
            skipped_existing
        );
        return Ok(skipped_existing);
    }

    let processed_penalties = penalties_to_submit.len() + skipped_existing;
    info!(
        "Submitting {} penalties to EVM (skipped {} already recorded)",
        penalties_to_submit.len(),
        skipped_existing
    );
    let submit_penalties_call = submit_penalties_contract.submit_penalties(penalties_to_submit);
    let pending_tx = submit_penalties_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    let receipt = pending_tx.await?;

    match receipt {
        Some(receipt) => {
            if receipt
                .status
                .map(|status| status.as_u64())
                .unwrap_or_default()
                != 1
            {
                return Err(anyhow!(
                    "Penalty submission transaction {:?} reverted with status {:?}",
                    tx_hash,
                    receipt.status
                ));
            }
            info!("Penalty submission successful. tx={:?}", tx_hash);
            Ok(processed_penalties)
        }
        None => Err(anyhow!(
            "Penalty submission transaction {:?} dropped without receipt",
            tx_hash
        )),
    }
}
