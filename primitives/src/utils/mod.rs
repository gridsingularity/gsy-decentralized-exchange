use anyhow::Result;
use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use chrono::{prelude::DateTime, Utc};
use std::env;
use std::str::FromStr;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

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

