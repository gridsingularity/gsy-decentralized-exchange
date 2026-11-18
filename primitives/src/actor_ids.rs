//! Keyed actor pseudonyms. Encoding a pseudonym as bytes16 is a separate operation.
use anyhow::{anyhow, Result};
use blake2::{
    digest::{consts::U16, KeyInit, Mac},
    Blake2bMac,
};
use std::sync::LazyLock;

/// Deliberately has no Debug implementation: the initialized MAC contains key material.
pub struct ActorIdHasher(Blake2bMac<U16>);

impl ActorIdHasher {
    pub fn from_hex_key(key: &str) -> Result<Self> {
        let mut bytes = [0; 32];
        hex::decode_to_slice(key, &mut bytes).map_err(|_| {
            anyhow!("ACTOR_ID_HASH_KEY must contain exactly 64 hexadecimal characters")
        })?;
        Ok(Self(
            <Blake2bMac<U16> as KeyInit>::new_from_slice(&bytes).expect("32-byte Blake2b key"),
        ))
    }

    /// Hashes the exact off-chain identity; no case folding or UUID interpretation.
    pub fn derive(&self, offchain_id: &str) -> String {
        let mut mac = self.0.clone();
        Mac::update(&mut mac, offchain_id.as_bytes());
        format!("0x{}", hex::encode(mac.finalize().into_bytes()))
    }
}

static HASHER: LazyLock<Result<ActorIdHasher>> = LazyLock::new(|| {
    let key = std::env::var("ACTOR_ID_HASH_KEY")
        .map_err(|_| anyhow!("ACTOR_ID_HASH_KEY is required and must be valid UTF-8"))?;
    ActorIdHasher::from_hex_key(&key)
});

/// Fail at startup, rather than when the first actor needs to be hashed.
pub fn initialize_actor_id_hashing() -> Result<()> {
    HASHER
        .as_ref()
        .map(|_| ())
        .map_err(|error| anyhow!("{error}"))
}

pub fn derive_onchain_id(offchain_id: &str) -> String {
    HASHER
        .as_ref()
        .expect("initialize ACTOR_ID_HASH_KEY at startup")
        .derive(offchain_id)
}
