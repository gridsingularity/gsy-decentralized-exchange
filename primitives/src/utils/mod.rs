use crate::MarketType;
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

pub fn generate_market_id(
    community_id: &str,
    market_type: MarketType,
    delivery_timestamp: u64,
) -> [u8; 16] {
    let mut market_id = [0u8; 16];
    let mut hasher = Blake2bVar::new(market_id.len()).expect("valid market ID output size");
    hasher.update(community_id.as_bytes());
    hasher.update(market_type.as_str().as_bytes());
    hasher.update(&delivery_timestamp.to_be_bytes());
    hasher
        .finalize_variable(&mut market_id)
        .expect("valid market ID output buffer");
    market_id
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

#[cfg(test)]
mod tests {
    use super::generate_market_id;
    use crate::MarketType;

    const COMMUNITY_ID: &str = "11111111-1111-4111-8111-111111111111";

    #[test]
    fn market_id_is_deterministic_for_the_same_seed() {
        let first = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);
        let second = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);

        assert_eq!(first, second);
    }

    #[test]
    fn market_id_changes_for_each_seed_component() {
        let market_id = generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_000);

        assert_ne!(
            market_id,
            generate_market_id(
                "22222222-2222-4222-8222-222222222222",
                MarketType::Spot,
                1_700_000_000,
            )
        );
        assert_ne!(
            market_id,
            generate_market_id(COMMUNITY_ID, MarketType::Flex, 1_700_000_000)
        );
        assert_ne!(
            market_id,
            generate_market_id(COMMUNITY_ID, MarketType::Spot, 1_700_000_001)
        );
    }
}
