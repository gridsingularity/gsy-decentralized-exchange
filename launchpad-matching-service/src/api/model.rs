use mongodb::{Client, Collection};
use crate::api::types::DbBidOfferMatch;
use std::env;
use async_trait::async_trait;

#[async_trait]
pub trait MatchStore: Send + Sync {
    async fn insert_matches(&self, matches: Vec<DbBidOfferMatch>) -> mongodb::error::Result<()>;
}

pub struct MatchModel {
    pub client: Client,
    pub db: mongodb::Database,
}

impl MatchModel {
    pub async fn new() -> mongodb::error::Result<Self> {
        let mongodb_uri = env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
        let client = Client::with_uri_str(mongodb_uri).await?;
        let db = client.database("launchpad");
        Ok(MatchModel { client, db })
    }
}

#[async_trait]
impl MatchStore for MatchModel {
    async fn insert_matches(&self, matches: Vec<DbBidOfferMatch>) -> mongodb::error::Result<()> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection("matches");

        if !matches.is_empty() {
            collection.insert_many(matches, None).await?;
        }

        Ok(())
    }
}
