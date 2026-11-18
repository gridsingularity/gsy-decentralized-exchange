use crate::MarketType;
use anyhow::{Result, anyhow};
use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use chrono::{prelude::DateTime, Utc, TimeZone};
use std::env;
use std::str::FromStr;
use uuid::Uuid;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub use crate::actor_ids::derive_onchain_id;

/// Entity UUIDs are v4; actor pseudonyms must not pass through this validator.
pub fn uuid_v4_to_bytes16(value: &str) -> Result<[u8; 16]> {
    let id = Uuid::parse_str(value).map_err(|_| anyhow!("invalid UUIDv4"))?;
    if !is_hyphenated_uuid(value)
        || id.get_version() != Some(uuid::Version::Random)
        || id.get_variant() != uuid::Variant::RFC4122
    {
        return Err(anyhow!("expected a hyphenated UUIDv4"));
    }
    Ok(*id.as_bytes())
}

pub fn bytes16_to_hex(value: [u8; 16]) -> String {
    format!("0x{}", hex::encode(value))
}

/// Formats 16 bytes as a lowercase hyphenated UUID (8-4-4-4-12).
pub fn bytes16_to_uuid_string(value: [u8; 16]) -> String {
    Uuid::from_bytes(value).to_string()
}

/// Accepts UUID (8-4-4-4-12), lowercase `0x`-prefixed hex, or bare bytes16 hex.
/// Does not trim, impose a UUID version, or reject optional all-zero sentinels.
pub fn parse_uuid_or_hex_bytes16(value: &str) -> Option<[u8; 16]> {
    let value = if is_hyphenated_uuid(value) {
        value.replace('-', "")
    } else {
        value.strip_prefix("0x").unwrap_or(value).to_string()
    };
    let mut bytes = [0; 16];
    hex::decode_to_slice(value, &mut bytes).ok()?;
    Some(bytes)
}

fn is_hyphenated_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Parses a required bytes16 identity, excluding the optional all-zero sentinel.
pub fn parse_nonzero_uuid_or_hex_bytes16(value: &str) -> Option<[u8; 16]> {
    parse_uuid_or_hex_bytes16(value).filter(|bytes| *bytes != [0; 16])
}

/// Validates an incoming actor identity without changing it.
pub fn normalize_offchain_id(value: &str) -> Result<String> {
    if value.is_empty() || value.trim() != value {
        return Err(anyhow!(
            "off-chain ID must be nonempty and have no surrounding whitespace"
        ));
    }
    Ok(value.to_string())
}

/// Canonical form of a community ID: the lowercase hyphenated UUID that
/// chain-sourced markets are stored with. Fails unless the ID is a nonzero UUID or
/// 16-byte hex value.
pub fn normalize_community_id(value: &str) -> Result<String> {
    parse_nonzero_uuid_or_hex_bytes16(value)
        .map(bytes16_to_uuid_string)
        .ok_or_else(|| {
            anyhow!(
                "community ID '{}' is not a nonzero UUID or 16-byte hex value",
                value
            )
        })
}

/// Canonical stored market ID: lowercase `0x` hex for a nonzero UUID or bytes16 value.
pub fn normalize_market_id(value: &str) -> Result<String> {
    parse_nonzero_uuid_or_hex_bytes16(value)
        .map(bytes16_to_hex)
        .ok_or_else(|| {
            anyhow!(
                "market ID '{}' is not a nonzero UUID or 16-byte hex value",
                value
            )
        })
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

pub fn profile_measurement_id(
    point_type: crate::db_api_schema::profiles::MeasurementPointType,
    community_uuid: &str,
    facility_id: &str,
) -> String {
    use crate::db_api_schema::profiles::MeasurementPointType;
    let prefix = match point_type {
        MeasurementPointType::Measurement => "measurement",
        MeasurementPointType::Forecast => "forecast",
    };
    format!(
        "{prefix}:{}:{}",
        if is_hyphenated_uuid(community_uuid) {
            community_uuid.to_ascii_lowercase()
        } else {
            community_uuid.to_string()
        },
        facility_id
    )
}

/// The community ID is normalized first, so every accepted form of one
/// community ID yields the same market ID. Fails for a community ID that is
/// not a nonzero UUID or 16-byte hex value.
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
