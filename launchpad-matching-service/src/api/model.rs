use mongodb::{Client, Collection, bson::doc};
use crate::api::types::DbBidOfferMatch;
use crate::configuration::get_configuration;
use serde::{Serialize, Deserialize};
use futures_util::StreamExt;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TimeSeriesPoint {
    pub time_slot: u64,
    pub average_energy_rate: f64,
}

pub struct MatchModel {
    pub client: Client,
    pub db: mongodb::Database,
    pub collection_name: String,
}

impl MatchModel {
    pub async fn new() -> mongodb::error::Result<Self> {
        let mongodb_uri = get_configuration().unwrap().get_connection_string();
        let client = Client::with_uri_str(mongodb_uri).await?;
        let db = client.database("launchpad");
        Ok(MatchModel { client, db, collection_name: "matches".to_string() })
    }

    pub fn with_collection(mut self, name: &str) -> Self {
        self.collection_name = name.to_string();
        self
    }

    pub async fn insert_matches(&self, matches: Vec<DbBidOfferMatch>) -> mongodb::error::Result<()> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection(&self.collection_name);

        if !matches.is_empty() {
            collection.insert_many(matches, None).await?;
        }

        Ok(())
    }

    pub async fn get_average_energy_rate_series(&self, market_id: Option<String>, start_time: u64, end_time: u64) -> mongodb::error::Result<Vec<TimeSeriesPoint>> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection(&self.collection_name);

        let mut match_filter = doc! {
            "time_slot": { "$gte": start_time as i64, "$lte": end_time as i64 }
        };

        if let Some(market_id) = market_id {
            match_filter.insert("market_id", market_id);
        }

        let pipeline = vec![
            doc! {
                "$match": match_filter
            },
            doc! {
                "$group": {
                    "_id": "$time_slot",
                    "average_energy_rate": { "$avg": "$energy_rate" }
                }
            },
            doc! {
                "$project": {
                    "time_slot": "$_id",
                    "average_energy_rate": 1,
                    "_id": 0
                }
            },
            doc! {
                "$sort": { "time_slot": 1 }
            }
        ];

        let mut cursor = collection.aggregate(pipeline, None).await?;
        let mut results = Vec::new();

        while let Some(result) = cursor.next().await {
            let doc = result?;
            let point: TimeSeriesPoint = mongodb::bson::from_document(doc)?;
            results.push(point);
        }

        Ok(results)
    }

    pub async fn get_matches(
        &self,
        start_time: u64,
        end_time: u64,
        market_id: Option<String>,
        limit: i64,
    ) -> mongodb::error::Result<Vec<DbBidOfferMatch>> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection(&self.collection_name);

        let mut filter = doc! {
            "time_slot": { "$gte": start_time as i64, "$lte": end_time as i64 }
        };

        if let Some(market_id) = market_id {
            filter.insert("market_id", market_id);
        }

        let options = mongodb::options::FindOptions::builder()
            .limit(limit)
            .sort(doc! { "time_slot": 1 })
            .build();

        let mut cursor = collection.find(filter, options).await?;
        let mut results = Vec::new();

        while let Some(result) = cursor.next().await {
            results.push(result?);
        }

        Ok(results)
    }
}
