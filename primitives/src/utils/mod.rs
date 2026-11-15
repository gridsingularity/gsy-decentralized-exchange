use crate::MarketType;
use anyhow::{Result, anyhow};
use blake2::digest::consts::U16;
use blake2::digest::{Mac, Update, VariableOutput};
use blake2::{Blake2bMac, Blake2bVar};
use chrono::{prelude::DateTime, Utc, TimeZone};
use once_cell::sync::Lazy;
use std::env;
use std::str::FromStr;
use uuid::Uuid;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub fn bytes16_to_hex(value: [u8; 16]) -> String {
    format!("0x{}", hex::encode(value))
}

/// Formats 16 bytes as a lowercase hyphenated UUID (8-4-4-4-12).
pub fn bytes16_to_uuid_string(value: [u8; 16]) -> String {
    Uuid::from_bytes(value).to_string()
}

pub fn parse_uuid_or_hex_bytes16(value: &str) -> Option<[u8; 16]> {
    let trimmed = value.trim();
    let hex_value = if let Some(stripped) = trimmed.strip_prefix("0x") {
        stripped.to_string()
    } else {
        trimmed.replace('-', "")
    };

    if hex_value.len() != 32 || !hex_value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let decoded = hex::decode(hex_value).ok()?;
    decoded.try_into().ok()
}

/// Canonical form of a community ID: the lowercase hyphenated UUID that
/// chain-sourced markets are stored with. Fails unless the ID is a UUID or
/// 16-byte hex value.
pub fn normalize_community_id(value: &str) -> Result<String> {
    parse_uuid_or_hex_bytes16(value)
        .map(bytes16_to_uuid_string)
        .ok_or_else(|| {
            anyhow!(
                "community ID '{}' is not a UUID or 16-byte hex value",
                value
            )
        })
}

/// Canonical stored market ID: lowercase `0x` hex for a UUID or 16-byte hex value.
pub fn normalize_market_id(value: &str) -> Result<String> {
    parse_uuid_or_hex_bytes16(value)
        .map(bytes16_to_hex)
        .ok_or_else(|| anyhow!("market ID '{}' is not a UUID or 16-byte hex value", value))
}

/// Environment variable holding the secret key of the on-chain ID hash.
pub const ONCHAIN_ID_HASH_KEY_ENV: &str = "ONCHAIN_ID_HASH_KEY";

static ONCHAIN_ID_HASH_KEY: Lazy<Vec<u8>> = Lazy::new(|| {
    let key = env::var(ONCHAIN_ID_HASH_KEY_ENV)
        .unwrap_or_default()
        .into_bytes();
    if key.len() > BLAKE2B_MAX_KEY_BYTES {
        panic!(
            "Invalid {}: the key is {} bytes, the maximum is {}",
            ONCHAIN_ID_HASH_KEY_ENV,
            key.len(),
            BLAKE2B_MAX_KEY_BYTES
        );
    }
    key
});

pub const BLAKE2B_MAX_KEY_BYTES: usize = 64;

pub fn create_encrypted_bytes16_from_string(input_string: &str) -> [u8; 16] {
    create_keyed_bytes16_from_string(input_string, &ONCHAIN_ID_HASH_KEY)
        .expect("on-chain ID hash key length is validated on first use")
}

pub fn create_keyed_bytes16_from_string(input_string: &str, key: &[u8]) -> Result<[u8; 16]> {
    if key.is_empty() {
        let mut hash = [0u8; 16];
        let mut hasher = Blake2bVar::new(16).expect("valid Blake2b output size");
        hasher.update(input_string.as_bytes());
        hasher
            .finalize_variable(&mut hash)
            .expect("valid Blake2b output buffer");
        return Ok(hash);
    }
    let mut hasher = Blake2bMac::<U16>::new_from_slice(key).map_err(|_| {
        anyhow!(
            "on-chain ID hash key is {} bytes, the maximum is {}",
            key.len(),
            BLAKE2B_MAX_KEY_BYTES
        )
    })?;
    Mac::update(&mut hasher, input_string.as_bytes());
    Ok(hasher.finalize().into_bytes().into())
}

/// The community ID is normalized first, so every accepted form of one
/// community ID yields the same market ID. Fails for a community ID that is
/// not a UUID or 16-byte hex value.
pub fn generate_market_id(
    community_id: &str,
    market_type: MarketType,
    delivery_timestamp: u64,
) -> Result<[u8; 16]> {
    let community_id = normalize_community_id(community_id)?;
    let mut market_id = [0u8; 16];
    let mut hasher = Blake2bVar::new(market_id.len()).expect("valid market ID output size");
    hasher.update(community_id.as_bytes());
    hasher.update(market_type.as_str().as_bytes());
    hasher.update(&delivery_timestamp.to_be_bytes());
    hasher
        .finalize_variable(&mut market_id)
        .expect("valid market ID output buffer");
    Ok(market_id)
}

pub fn timestamp_to_datetime_string(timestamp: u64) -> String {
    let datetime = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap();
    // Formats the combined date and time with the specified format string.
    datetime.format("%Y-%m-%d %H:%M:%S.%f").to_string()
}

pub fn read_env_or<T: FromStr>(variable_name: &str, default_value: T) -> T {
    env::var(variable_name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_value)
}

pub fn timestamp_to_string_with_padding(timestamp: u64) -> String {
    format!("{:020}", timestamp)
}

pub fn string_to_timestamp(timestamp_string: &str) -> Result<u64> {
    let ts: u64 = timestamp_string.parse()?;
    Ok(ts)
}

/// Format epoch seconds as an ISO 8601 / RFC 3339 string with timezone (UTC, `Z`).
pub fn epoch_to_rfc3339(epoch_secs: u64) -> String {
    Utc.timestamp_opt(epoch_secs as i64, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

pub fn rfc3339_to_epoch(value: &str) -> Result<u64> {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|e| anyhow!("invalid ISO 8601 timestamp '{}': {}", value, e))?;
    Ok(parsed.timestamp() as u64)
}
pub fn opt_rfc3339_to_epoch(value: Option<&str>) -> Result<Option<u64>> {
    value.map(rfc3339_to_epoch).transpose()
}
