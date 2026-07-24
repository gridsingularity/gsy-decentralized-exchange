use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IdType {
    #[serde(rename = "actor_id")]
    ActorId,
    #[serde(rename = "oder_id")]
    OrderId,
    #[serde(rename = "trade_id")]
    TradeId,
    #[serde(rename = "market_id")]
    MarketId,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IdMappingSchema{
    pub offchain_id: String,
    pub onchain_id: String,
    pub id_type: IdType,
    pub creation_time: u64,
}