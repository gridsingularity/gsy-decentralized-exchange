use actix_web::web;
use anyhow::Result;
use mongodb::options::ClientOptions;
use mongodb::Database;
use std::ops::Deref;
use crate::db::order_service::{init, OrderService};
use crate::db::trade_service::TradeService;

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
    init(db).await?;
    Ok(())
}
