use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IdMappingSchema{
    pub offchain_id: String,
    pub onchain_id: String,
    pub creation_time: u64,
}