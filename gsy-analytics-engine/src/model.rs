//! Plain input records the KPIs compute from. The database readers build them from the
//! offchain-storage collections, so the KPI code never touches MongoDB.

pub use primitives::db_api_schema::trades::TradeStatus;

/// A settled P2P trade.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRecord {
    pub trade_uuid: String,
    /// Community whose market the trade was cleared on. `None` if no known community matches.
    pub community_id: Option<String>,
    /// On-chain facility id of the buyer (bytes16 hex).
    pub buyer: String,
    /// Delivery start, unix seconds.
    pub time_slot: i64,
    pub energy_kwh: f64,
    /// Clearing price in EUR/kWh.
    pub energy_rate: f64,
    pub status: TradeStatus,
}

/// Net metered energy of one facility in one slot.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterReading {
    pub community_id: String,
    /// On-chain facility id (bytes16 hex), the same id trades use for `buyer`.
    pub facility: String,
    /// Slot start, unix seconds.
    pub time_slot: i64,
    /// Positive for import (consumption), negative for export.
    pub energy_kwh: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommunityRef {
    pub community_id: String,
    pub community_name: String,
}

/// Everything loaded from MongoDB for one tick, shared by all KPIs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dataset {
    pub communities: Vec<CommunityRef>,
    pub trades: Vec<TradeRecord>,
    pub readings: Vec<MeterReading>,
}
