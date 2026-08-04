use crate::types::{AccountId32, H256};
use anyhow::Result;
use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use chrono::{prelude::DateTime, Utc};
use std::env;
use std::str::FromStr;
use thiserror::Error;
use crate::MarketType;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub fn h256_to_string(hash: H256) -> String {
    format!("0x{}", hex::encode(hash.as_bytes()))
}

pub fn string_to_h256(hex_string: String) -> H256 {
    let hex_stripped = hex_string
        .strip_prefix("0x")
        .expect("H256 string must start with 0x");
    let bytes = hex::decode(hex_stripped).expect("Invalid hex");
    H256::from_slice(&bytes)
}

pub fn string_to_account_id(account_id_str: String) -> Option<AccountId32> {
    AccountId32::from_str(&account_id_str).ok()
}

pub fn evm_address_to_account_id(evm_address: &str) -> Option<AccountId32> {
    let trimmed = evm_address.trim();
    let hex = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if hex.len() != 40 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let raw = hex::decode(hex).ok()?;
    if raw.len() != 20 {
        return None;
    }

    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(&raw);
    Some(AccountId32::from(padded))
}

pub fn bytes16_to_hex(value: [u8; 16]) -> String {
    format!("0x{}", hex::encode(value))
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

pub fn parse_or_hash_bytes16(value: &str) -> [u8; 16] {
    if let Some(parsed) = parse_uuid_or_hex_bytes16(value) {
        return parsed;
    }

    let mut hash = [0u8; 32];
    let mut hasher = Blake2bVar::new(32).expect("valid Blake2b output size");
    hasher.update(value.as_bytes());
    hasher
        .finalize_variable(&mut hash)
        .expect("valid Blake2b output buffer");
    hash[0..16]
        .try_into()
        .expect("blake2 hash prefix is 16 bytes")
}

pub fn bytes16_to_h256(value: [u8; 16]) -> H256 {
    let mut padded = [0u8; 32];
    padded[..16].copy_from_slice(&value);
    H256::from(padded)
}

pub fn h256_to_bytes16_hex(value: H256) -> String {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&value.as_bytes()[..16]);
    bytes16_to_hex(bytes)
}

pub fn actor_id_to_account_id(value: &str) -> Option<AccountId32> {
    let actor_id = parse_uuid_or_hex_bytes16(value)?;
    let mut padded = [0u8; 32];
    padded[..16].copy_from_slice(&actor_id);
    Some(AccountId32::from(padded))
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



#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("invalid byte length")]
    InvalidByteLength,
    #[error("failed to parse byte to utf-8")]
    FailedToParseByte,
    #[error("missing encryption key")]
    MissingKey,
    #[error("invalid encryption key")]
    InvalidKey,
    #[error("invalid encryption key length")]
    InvalidKeyLength,
}

/// Packs a string into a fixed 16-byte buffer, right-padded with zeros.
///
/// The string's UTF-8 bytes are copied into the start of the buffer; any
/// remaining bytes are left as zero.
pub fn string_to_bytes16(s: &str) -> Result<[u8; 16], ConvertError> {
    let mut buf = [0u8; 16];
    let src = s.as_bytes();
    if src.len() > 16 {
        return Err(ConvertError::InvalidByteLength);
    }
    let n = src.len().min(16);
    buf[..n].copy_from_slice(&src[..n]);
    Ok(buf)
}

/// Converts a 16-byte buffer back into a string, stripping zero padding.
///
/// Reads up to the first zero byte (the padding boundary written by
/// [`string_to_bytes16`]) and interprets the preceding bytes as UTF-8.
pub fn bytes16_to_string(buf: &[u8; 16]) -> Result<String, ConvertError>{
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8(buf[..end].to_vec()).map_err(|_| ConvertError::FailedToParseByte)
}

pub fn create_encrypted_bytes16_from_string(input_string: &str) -> [u8; 16] {
    let mut hash = [0u8; 16];
    let mut hasher = Blake2bVar::new(16).expect("valid Blake2b output size");
    hasher.update(input_string.as_bytes());
    hasher
        .finalize_variable(&mut hash)
        .expect("valid Blake2b output buffer");
    hash
}

pub fn generate_market_id(market_type: MarketType, delivery_timestamp: u64) -> [u8; 16] {
    let offchain_id =  format!("{} {}", market_type.as_str(), delivery_timestamp);
    create_encrypted_bytes16_from_string(&offchain_id)
}