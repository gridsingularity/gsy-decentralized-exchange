use anyhow::{anyhow, Result};
use chrono::{prelude::DateTime, TimeZone, Utc};
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

/// Canonical off-chain entity ID. Actor identities must not use this normalizer.
pub fn normalize_uuid(value: &str) -> Result<String> {
    uuid_v4_to_bytes16(value).map(bytes16_to_uuid_string)
}

/// Decodes an on-chain actor pseudonym, not an off-chain identity or entity UUID.
pub fn hex_to_bytes16(value: &str) -> Option<[u8; 16]> {
    let mut bytes = [0; 16];
    hex::decode_to_slice(value.strip_prefix("0x")?, &mut bytes).ok()?;
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

/// Validates an incoming actor identity without changing it.
pub fn normalize_offchain_id(value: &str) -> Result<String> {
    if value.is_empty() || value.trim() != value {
        return Err(anyhow!(
            "off-chain ID must be nonempty and have no surrounding whitespace"
        ));
    }
    Ok(value.to_string())
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
    // Actor IDs may contain the separator; escape components, not the stored IDs.
    let component = |id: &str| id.replace('%', "%25").replace(':', "%3A");
    format!(
        "{prefix}:{}:{}",
        component(community_uuid),
        component(facility_id)
    )
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

/// Format epoch seconds as an ISO 8601 / RFC 3339 string with timezone (UTC, `+00:00`).
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

/// The current time in unix seconds.
pub fn now_epoch_secs() -> u64 {
    Utc::now().timestamp().max(0) as u64
}

/// `valid_from` of an entry whose receive time is unknown: `epoch_to_rfc3339(0)`, the oldest
/// possible.
pub const EPOCH_RFC3339: &str = "1970-01-01T00:00:00+00:00";
