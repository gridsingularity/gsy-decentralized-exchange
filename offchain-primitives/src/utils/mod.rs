use chrono::{prelude::DateTime, Utc};
use sp_core::H256;
use sp_runtime::AccountId32;
use std::env;
use std::str::FromStr;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub fn h256_to_string(hash: H256) -> String {
    format!("{:?}", hash)
}

pub fn string_to_h256(hex_string: String) -> H256 {
    let hex_stripped = &hex_string[2..];
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
