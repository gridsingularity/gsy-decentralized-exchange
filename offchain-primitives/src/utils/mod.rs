use subxt::utils::{AccountId32, H256};
use std::str::FromStr;
use chrono::{Utc, prelude::DateTime};
use std::env;


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
