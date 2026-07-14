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
    db.forecasts().0.ensure_id_index().await?;
    db.measurements().0.ensure_id_index().await?;
    db.markets().0.ensure_id_index().await?;
    Ok(())
}
