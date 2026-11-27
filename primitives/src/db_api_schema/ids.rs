use bson::serde_helpers::chrono_datetime_as_bson_datetime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IdMappingSchema {
    pub offchain_id: String,
    pub onchain_id: String,
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub creation_time: DateTime<Utc>,
}
