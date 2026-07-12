use gsy_offchain_primitives::db_api_schema::market::MarketTopologySchema;
use gsy_offchain_primitives::db_api_schema::orders::DbOrderSchema;
use gsy_offchain_primitives::db_api_schema::profiles::{ForecastSchema, MeasurementSchema};
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use std::sync::{Arc, RwLock};

/// Shared, thread-safe in-memory collection used as a drop-in replacement for a
/// MongoDB `Collection<T>` in tests.
pub type InMemoryCollection<T> = Arc<RwLock<Vec<T>>>;

/// In-memory storage backend mirroring the MongoDB collections, so the API and
/// its tests can run without a running MongoDB instance.
#[derive(Clone, Default)]
pub struct InMemoryDb {
    pub(crate) orders: InMemoryCollection<DbOrderSchema>,
    pub(crate) trades: InMemoryCollection<TradeSchema>,
    pub(crate) measurements: InMemoryCollection<MeasurementSchema>,
    pub(crate) forecasts: InMemoryCollection<ForecastSchema>,
    pub(crate) markets: InMemoryCollection<MarketTopologySchema>,
}
