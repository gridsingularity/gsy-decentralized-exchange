use crate::db::grid_topology_service::{
    init_assets, init_communities, init_facilities, init_pilot_sites, init_sites, AssetService,
    EnergyCommunityService, FacilityService, PilotSiteService, SiteService,
};
use crate::db::market_service::{init_markets, MarketService};
use crate::db::measurements_service::{
    init_measurement_points, init_timeseries, MeasurementPointService, TimeseriesService,
};
use crate::db::order_service::{
    init_flexibility_orders, init_orders, init_tariffs, FlexibilityOrderService, OrderService,
    TariffService,
};
use crate::db::trade_service::{
    init_clearing_results, init_market_roles, init_trades, ClearingResultService,
    MarketRoleService, TradeService,
};
use actix_web::web;
use anyhow::Result;
use mongodb::options::ClientOptions;
use mongodb::Database;
use std::ops::Deref;

pub type DbRef = web::Data<DatabaseWrapper>;

#[derive(Clone)]
#[repr(transparent)]
pub struct DatabaseWrapper(pub Database);

impl DatabaseWrapper {
    // Order Book Storage (D3.2 §5.4)
    pub fn orders(&self) -> OrderService {
        self.into()
    }
    pub fn flexibility_orders(&self) -> FlexibilityOrderService {
        self.into()
    }
    pub fn tariffs(&self) -> TariffService {
        self.into()
    }

    // Trades Storage (D3.2 §5.3)
    pub fn trades(&self) -> TradeService {
        self.into()
    }
    pub fn clearing_results(&self) -> ClearingResultService {
        self.into()
    }
    pub fn market_roles(&self) -> MarketRoleService {
        self.into()
    }
    pub fn markets(&self) -> MarketService {
        self.into()
    }

    // Measurements Storage (D3.2 §5.2)
    pub fn measurement_points(&self) -> MeasurementPointService {
        self.into()
    }
    pub fn timeseries(&self) -> TimeseriesService {
        self.into()
    }

    // Grid Topology and Market Storage (D3.2 §5.1)
    pub fn assets(&self) -> AssetService {
        self.into()
    }
    pub fn pilot_sites(&self) -> PilotSiteService {
        self.into()
    }
    pub fn communities(&self) -> EnergyCommunityService {
        self.into()
    }
    pub fn sites(&self) -> SiteService {
        self.into()
    }
    pub fn facilities(&self) -> FacilityService {
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

pub async fn delete_database(db_url: String, db_name: String) -> Result<()> {
    let options = ClientOptions::parse(&db_url).await?;
    let client = mongodb::Client::with_options(options)?;
    client.database(db_name.as_str()).drop().await?;
    Ok(())
}

async fn preload(db: &DatabaseWrapper) -> Result<()> {
    init_orders(db).await?;
    init_flexibility_orders(db).await?;
    init_tariffs(db).await?;
    init_trades(db).await?;
    init_clearing_results(db).await?;
    init_market_roles(db).await?;
    init_markets(db).await?;
    init_measurement_points(db).await?;
    init_timeseries(db).await?;
    init_assets(db).await?;
    init_pilot_sites(db).await?;
    init_communities(db).await?;
    init_sites(db).await?;
    init_facilities(db).await?;
    Ok(())
}
