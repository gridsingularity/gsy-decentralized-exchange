use crate::db::asset_measurements_service::AssetMeasurementsService;
use crate::db::collection::Coll;
use crate::db::forecasts_service::ForecastsService;
use crate::db::in_memory::{InMemoryCollection, InMemoryDb};
use crate::db::market_service::MarketService;
use crate::db::measurements_service::MeasurementsService;
use crate::db::order_service::OrderService;
use crate::db::trade_service::TradeService;
use actix_web::web;
use anyhow::Result;
use mongodb::Database;
use mongodb::options::ClientOptions;

pub type DbRef = web::Data<DatabaseWrapper>;

/// Storage backend used by the API: either a real MongoDB database or an
/// in-memory store (used by tests to run without MongoDB).
#[derive(Clone)]
pub enum DatabaseWrapper {
    Mongo(Database),
    InMemory(InMemoryDb),
}

impl DatabaseWrapper {
    /// Create an in-memory backend, e.g. for tests that should not require MongoDB.
    pub fn in_memory() -> Self {
        DatabaseWrapper::InMemory(InMemoryDb::default())
    }

    /// The single point where the storage backend is selected for a collection.
    pub(crate) fn coll<T: Send + Sync>(
        &self,
        name: &str,
        mem_collection: impl Fn(&InMemoryDb) -> InMemoryCollection<T>,
    ) -> Coll<T> {
        match self {
            DatabaseWrapper::Mongo(database) => Coll::Mongo(database.collection(name)),
            DatabaseWrapper::InMemory(store) => Coll::InMemory(mem_collection(store)),
        }
    }

    pub fn orders(&self) -> OrderService {
        self.into()
    }
    pub fn trades(&self) -> TradeService {
        self.into()
    }
    pub fn measurements(&self) -> MeasurementsService {
        self.into()
    }
    pub fn asset_measurements(&self) -> AssetMeasurementsService {
        self.into()
    }
    pub fn forecasts(&self) -> ForecastsService {
        self.into()
    }
    pub fn markets(&self) -> MarketService {
        self.into()
    }
}

pub async fn init_database(db_url: String, db_name: String) -> Result<DatabaseWrapper> {
    let options = ClientOptions::parse(&db_url).await?;
    let client = mongodb::Client::with_options(options)?;
    let db = DatabaseWrapper::Mongo(client.database(db_name.as_str()));
    preload(&db).await?;
    Ok(db)
}

async fn preload(db: &DatabaseWrapper) -> Result<()> {
    // put initialize here
    db.orders().0.ensure_id_index().await?;
    db.trades().0.ensure_id_index().await?;
    // Backs the trade_uuid key used by `insert_trades`. Trades re-posted before that key existed
    // were stored once per post, which would fail the index build, so the copies are removed
    // first, keeping a judged copy over a `Settled` one (see `remove_duplicate_trades`). As for
    // the measurements below, neither step may stop the service.
    match db.trades().remove_duplicate_trades().await {
        Ok(0) => tracing::info!("Found no duplicate trade_uuid trades"),
        Ok(removed) => tracing::warn!(
            "Removed {} duplicate trade_uuid trades, keeping a judged copy over a Settled one, else the oldest",
            removed
        ),
        Err(e) => tracing::error!(
            "Failed to remove duplicate trade_uuid trades: {:?}. Continuing; while duplicates remain the unique index on trade_uuid cannot be built, and inserts still skip stored trades but scan the collection",
            e
        ),
    }
    if let Err(e) = db
        .trades()
        .0
        .ensure_unique_index(mongodb::bson::doc! {"trade_uuid": 1})
        .await
    {
        tracing::error!(
            "Failed to create the unique trade_uuid index on trades: {:?}. Continuing without it; inserts still skip stored trades but scan the collection",
            e
        );
    }
    db.forecasts().0.ensure_id_index().await?;
    // Backs the (area_uuid, time_slot) upsert key used by `insert_forecasts`.
    db.forecasts()
        .0
        .ensure_unique_index(mongodb::bson::doc! {"area_uuid": 1, "time_slot": 1})
        .await?;
    db.measurements().0.ensure_id_index().await?;
    // Backs the (area_hash, time_slot) upsert key used by `insert_measurements`. Rows stored
    // before that upsert existed may repeat a key, which would fail the index build, so they are
    // removed first, keeping the newest `creation_time` of each key. Neither step may stop the
    // service: a failure is logged and start-up continues, if need be without the index.
    match db
        .measurements()
        .0
        .remove_duplicates(&["area_hash", "time_slot"], "creation_time")
        .await
    {
        Ok(0) => tracing::info!("Found no duplicate (area_hash, time_slot) measurements"),
        Ok(removed) => tracing::warn!(
            "Removed {} duplicate (area_hash, time_slot) measurements, keeping the newest creation_time of each",
            removed
        ),
        Err(e) => tracing::error!(
            "Failed to remove duplicate (area_hash, time_slot) measurements: {:?}. Continuing; while duplicates remain the unique index on them cannot be built, and upserts still work but scan the collection",
            e
        ),
    }
    if let Err(e) = db
        .measurements()
        .0
        .ensure_unique_index(mongodb::bson::doc! {"area_hash": 1, "time_slot": 1})
        .await
    {
        tracing::error!(
            "Failed to create the unique (area_hash, time_slot) index on measurements: {:?}. Continuing without it; upserts still work but scan the collection",
            e
        );
    }
    db.markets().0.ensure_id_index().await?;
    Ok(())
}
