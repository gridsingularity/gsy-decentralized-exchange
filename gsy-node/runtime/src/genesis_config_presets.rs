// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use frame_support::__private::serde_json;
use crate::{AccountId, SessionKeys};
use alloc::{vec, vec::Vec};
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_genesis_builder::{self, PresetId};
use sp_keyring::Sr25519Keyring;

/// Identifier of the PoA genesis preset (`--chain poa`).
#[cfg(feature = "poa")]
pub const POA_RUNTIME_PRESET: &str = "poa";

// Returns the genesis config presets populated with given parameters.
//
// Each authority is described by its `(account, aura_key, grandpa_key)`. The Aura
// and GRANDPA authority sets are no longer seeded directly; instead they are
// derived from the `session` keys at genesis and thereafter managed by the
// `validator-set` pallet, which lets validators be added/removed on a live chain.
fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
    endowed_accounts: Vec<AccountId>,
    root: AccountId,
) -> Value {
    // (account, validator_id, session_keys) — validator_id == account here.
    let session_keys = initial_authorities
        .iter()
        .map(|(account, aura, grandpa)| {
            (
                account.clone(),
                account.clone(),
                SessionKeys { aura: aura.clone(), grandpa: grandpa.clone() },
            )
        })
        .collect::<Vec<_>>();
    let initial_validators = initial_authorities
        .iter()
        .map(|(account, _, _)| account.clone())
        .collect::<Vec<_>>();

    serde_json::json!({
		"balances": {
			// Configure endowed accounts with initial balance of 1 << 60.
			"balances": endowed_accounts.iter().cloned().map(|k| (k, 1u64 << 60)).collect::<Vec<_>>(),
		},
		"session": {
			// Registers the session keys and seeds the initial Aura/GRANDPA sets.
			"keys": session_keys,
		},
		"validatorSet": {
			// The initial active validator set (must match the session keys above).
			"initialValidators": initial_validators,
		},
		"sudo": {
			// Assign network admin rights.
			"key": Some(root),
		},
	})
}


/// Return the development genesis config.
pub fn development_config_genesis() -> Value {
    testnet_genesis(
        vec![(
            Sr25519Keyring::Alice.to_account_id(),
            sp_keyring::Sr25519Keyring::Alice.public().into(),
            sp_keyring::Ed25519Keyring::Alice.public().into(),
        )],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::Charlie.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
        ],
        sp_keyring::Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Return the local genesis config preset.
pub fn local_config_genesis() -> Value {
    testnet_genesis(
        vec![
            (
                Sr25519Keyring::Alice.to_account_id(),
                sp_keyring::Sr25519Keyring::Alice.public().into(),
                sp_keyring::Ed25519Keyring::Alice.public().into(),
            ),
            (
                Sr25519Keyring::Bob.to_account_id(),
                sp_keyring::Sr25519Keyring::Bob.public().into(),
                sp_keyring::Ed25519Keyring::Bob.public().into(),
            ),
        ],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::Charlie.to_account_id(),
            Sr25519Keyring::Dave.to_account_id(),
            Sr25519Keyring::Eve.to_account_id(),
            Sr25519Keyring::Ferdie.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
            Sr25519Keyring::CharlieStash.to_account_id(),
            Sr25519Keyring::DaveStash.to_account_id(),
            Sr25519Keyring::EveStash.to_account_id(),
            Sr25519Keyring::FerdieStash.to_account_id(),
        ],
        Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Return the PoA genesis config preset.
///
/// The network is seeded with a **2-validator** Aura+GRANDPA set using the
/// well-known Alice/Bob keys, and is designed to grow: further validators can be
/// added on the live chain via `ValidatorSet::add_validator` (sudo). For a real
/// deployment with externally generated keys, build a chain spec from this
/// preset and edit the `session.keys` / `validatorSet.initialValidators` /
/// `sudo.key` fields instead. See `docs/setup/deployment.md`.
#[cfg(feature = "poa")]
pub fn poa_config_genesis() -> Value {
    testnet_genesis(
        vec![
            (
                Sr25519Keyring::Alice.to_account_id(),
                sp_keyring::Sr25519Keyring::Alice.public().into(),
                sp_keyring::Ed25519Keyring::Alice.public().into(),
            ),
            (
                Sr25519Keyring::Bob.to_account_id(),
                sp_keyring::Sr25519Keyring::Bob.public().into(),
                sp_keyring::Ed25519Keyring::Bob.public().into(),
            ),
        ],
        vec![
            Sr25519Keyring::Alice.to_account_id(),
            Sr25519Keyring::Bob.to_account_id(),
            Sr25519Keyring::Charlie.to_account_id(),
            Sr25519Keyring::AliceStash.to_account_id(),
            Sr25519Keyring::BobStash.to_account_id(),
            Sr25519Keyring::CharlieStash.to_account_id(),
        ],
        Sr25519Keyring::Alice.to_account_id(),
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        #[cfg(feature = "poa")]
        POA_RUNTIME_PRESET => poa_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
        #[cfg(feature = "poa")]
        PresetId::from(POA_RUNTIME_PRESET),
    ]
}