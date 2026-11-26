use anyhow::{anyhow, Result};
use chrono::{prelude::DateTime, SubsecRound, Utc};
use std::env;
use std::str::FromStr;
use uuid::Uuid;

pub const NODE_FLOAT_SCALING_FACTOR: f64 = 10000.0;

pub use crate::actor_ids::derive_onchain_id;

/// Entity UUIDs are v4; actor pseudonyms must not pass through this validator.
pub fn uuid_v4_to_bytes16(value: &str) -> Result<[u8; 16]> {
    let id = Uuid::parse_str(value).map_err(|_| anyhow!("invalid UUIDv4"))?;
    if value.len() != uuid::fmt::Hyphenated::LENGTH
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

/// Validates an incoming actor identity without changing it.
pub fn validate_offchain_actor_id(value: &str) -> Result<String> {
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

pub fn read_env_or<T: FromStr>(variable_name: &str, default_value: T) -> T {
    env::var(variable_name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_value)
}

/// Format epoch seconds as an ISO 8601 / RFC 3339 string in UTC (`+00:00`), the same
/// form as `DateTime::<Utc>::to_rfc3339`, which the DTOs use. Empty if out of range.
pub fn epoch_to_rfc3339(epoch_secs: u64) -> String {
    epoch_to_datetime(epoch_secs)
        .map(|datetime| datetime.to_rfc3339())
        .unwrap_or_default()
}

pub fn rfc3339_to_epoch(value: &str) -> Result<u64> {
    datetime_to_epoch(&rfc3339_to_datetime(value)?)
}

/// Parses an ISO 8601 / RFC 3339 string with any offset into UTC, truncated to milliseconds,
/// the precision of a BSON date, so that a parsed time equals its stored value.
pub fn rfc3339_to_datetime(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc).trunc_subsecs(3))
        .map_err(|e| anyhow!("invalid ISO 8601 timestamp '{}': {}", value, e))
}
pub fn opt_rfc3339_to_datetime(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value.map(rfc3339_to_datetime).transpose()
}

/// Serde for a UTC date-time as an RFC 3339 string: written like `to_rfc3339` (`+00:00`), the
/// form of the DTOs, and read with `rfc3339_to_datetime`.
pub mod rfc3339 {
    use chrono::{DateTime, Utc};
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        time: &DateTime<Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&time.to_rfc3339())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        super::rfc3339_to_datetime(&String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Epoch seconds (e.g. a block timestamp) as a UTC date-time.
pub fn epoch_to_datetime(epoch_secs: u64) -> Result<DateTime<Utc>> {
    i64::try_from(epoch_secs)
        .ok()
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .ok_or_else(|| anyhow!("epoch timestamp {} is out of range", epoch_secs))
}

/// Whole epoch seconds of a UTC date-time; fails for times before 1970.
pub fn datetime_to_epoch(datetime: &DateTime<Utc>) -> Result<u64> {
    u64::try_from(datetime.timestamp())
        .map_err(|_| anyhow!("{} is before the Unix epoch", datetime.to_rfc3339()))
}
