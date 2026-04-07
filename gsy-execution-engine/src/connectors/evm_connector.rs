use crate::primitives::penalty_calculator::Penalty;
use anyhow::{anyhow, Result};
use ethers::prelude::*;
use ethers::utils::keccak256;
use std::{str::FromStr, sync::Arc};
use tracing::{info, warn};

abigen!(
    TradeSettlementContract,
    r#"[
        {
            "type": "function",
            "name": "hasRole",
            "stateMutability": "view",
            "inputs": [
                {"name": "role", "type": "bytes32"},
                {"name": "account", "type": "address"}
            ],
            "outputs": [{"name": "", "type": "bool"}]
        },
        {
            "type": "function",
            "name": "submitPenalties",
            "stateMutability": "nonpayable",
            "inputs": [
                {
                    "name": "penalties",
                    "type": "tuple[]",
                    "components": [
                        {"name": "penalizedAccount", "type": "address"},
                        {"name": "marketId", "type": "bytes32"},
                        {"name": "tradeId", "type": "bytes32"},
                        {"name": "penaltyEnergy", "type": "uint64"}
                    ]
                }
            ],
            "outputs": []
        },
        {
            "type": "function",
            "name": "penaltyEnergyByTrade",
            "stateMutability": "view",
            "inputs": [{"name": "tradeId", "type": "bytes32"}],
            "outputs": [{"name": "", "type": "uint256"}]
        }
    ]"#
);

fn parse_or_hash_bytes32(id: &str) -> [u8; 32] {
    if id.starts_with("0x") && id.len() == 66 {
        if let Ok(parsed) = H256::from_str(id) {
            return parsed.to_fixed_bytes();
        }
    }
    keccak256(id.as_bytes())
}

type EvmPenaltyTuple = (Address, [u8; 32], [u8; 32], u64);

fn to_evm_penalties(penalties: Vec<Penalty>) -> Vec<EvmPenaltyTuple> {
    penalties
        .into_iter()
        .filter_map(|penalty| {
            let penalized_account = match Address::from_str(&penalty.penalized_account) {
                Ok(account) => account,
                Err(_) => {
                    warn!(
                        "Skipping penalty with invalid account '{}'",
                        penalty.penalized_account
                    );
                    return None;
                }
            };

            if penalty.penalty_cost == 0 {
                warn!(
                    "Skipping penalty for trade '{}' because penalty_cost is zero",
                    penalty.trade_uuid
                );
                return None;
            }

            Some((
                penalized_account,
                parse_or_hash_bytes32(&penalty.market_id),
                parse_or_hash_bytes32(&penalty.trade_uuid),
                penalty.penalty_cost,
            ))
        })
        .collect()
}

pub async fn submit_penalties(
    evm_node_url: &str,
    trade_settlement_address: &str,
    execution_engine_private_key: &str,
    penalties: Vec<Penalty>,
) -> Result<()> {
    if penalties.is_empty() {
        info!("No penalties to submit.");
        return Ok(());
    }

    let trade_settlement_address = Address::from_str(trade_settlement_address).map_err(|e| {
        anyhow!(
            "Invalid trade settlement address '{}': {}",
            trade_settlement_address,
            e
        )
    })?;

    let evm_penalties = to_evm_penalties(penalties);
    if evm_penalties.is_empty() {
        info!("No valid penalties to submit after validation.");
        return Ok(());
    }

    let provider = Provider::<Ws>::connect(evm_node_url).await?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let wallet = execution_engine_private_key
        .parse::<LocalWallet>()
        .map_err(|e| anyhow!("Invalid execution engine private key: {}", e))?
        .with_chain_id(chain_id);
    let signer_address = wallet.address();

    let client = Arc::new(SignerMiddleware::new(provider, wallet));
    let trade_settlement = TradeSettlementContract::new(trade_settlement_address, client.clone());

    let execution_engine_role = keccak256("EXECUTION_ENGINE_ROLE");
    let has_role = trade_settlement
        .has_role(execution_engine_role, signer_address)
        .call()
        .await?;
    if !has_role {
        warn!(
            "Signer {:?} does not currently have EXECUTION_ENGINE_ROLE in TradeSettlement",
            signer_address
        );
    }

    let mut penalties_to_submit: Vec<EvmPenaltyTuple> = Vec::new();
    let mut skipped_existing = 0usize;
    for penalty in evm_penalties {
        let existing = trade_settlement
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
        return Ok(());
    }

    info!(
        "Submitting {} penalties to EVM (skipped {} already recorded)",
        penalties_to_submit.len(),
        skipped_existing
    );
    let submit_penalties_call = trade_settlement.submit_penalties(penalties_to_submit);
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
            Ok(())
        }
        None => Err(anyhow!(
            "Penalty submission transaction {:?} dropped without receipt",
            tx_hash
        )),
    }
}
