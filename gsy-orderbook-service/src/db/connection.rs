use crate::db::forecasts_service::{ForecastsService, init_forecasts};
use crate::db::market_service::{MarketService, init_markets};
use crate::db::measurements_service::{MeasurementsService, init_measurements};
use crate::db::order_service::{OrderService, init_orders};
use crate::db::trade_service::{TradeService, init_trades};
use actix_web::web;
use anyhow::Result;
use mongodb::Database;
use mongodb::options::ClientOptions;
use std::ops::Deref;

pub type DbRef = web::Data<DatabaseWrapper>;

#[derive(Clone)]
#[repr(transparent)]
pub struct DatabaseWrapper(pub Database);

impl DatabaseWrapper {
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

impl Deref for DatabaseWrapper {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn init_database(db_url: String, db_name: String) -> Result<DatabaseWrapper> {
    let options = ClientOptions::parse(&db_url).await?;
    let client = mongodb::Client::with_options(options)?;
    let db = DatabaseWrapper(client.database(db_name.as_str()));
    preload(&db).await?;
    Ok(db)
}

async fn preload(db: &DatabaseWrapper) -> Result<()> {
    // put initialize here
    init_orders(db).await?;
    init_trades(db).await?;
    init_forecasts(db).await?;
    init_measurements(db).await?;
    init_markets(db).await?;
    Ok(())
}
