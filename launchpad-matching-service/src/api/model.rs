use crate::api::controller::DbMarketData;
use crate::api::types::DbBidOfferMatch;
use crate::configuration::get_configuration;
use futures_util::StreamExt;
use mongodb::bson::{Bson, Document, doc};
use mongodb::error::ErrorKind;
use mongodb::options::{FindOptions, IndexOptions, UpdateOptions};
use mongodb::{Client, Collection, Database, IndexModel};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// One point per `time_slot`.
    NoAggregation,
    /// 15-minute buckets aligned to :00, :15, :30 and :45 in the time zone.
    QuarterHour,
    /// Calendar days starting at local midnight in the time zone.
    Day,
    /// Calendar months starting at local midnight of the 1st in the time zone.
    Month,
}

/// Sort direction of `/matches`, applied to `time_slot` and then `match_id`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Copy, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl SortOrder {
    fn direction(self) -> i32 {
        match self {
            SortOrder::Asc => 1,
            SortOrder::Desc => -1,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TimeSeriesPoint {
    pub time_slot: u64,
    pub average_energy_rate: f64,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EnergyTimeSeriesPoint {
    pub time_slot: u64,
    pub matched_energy_kWh: f64,
    pub unmatched_energy_kWh: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MarketStatisticsResponse {
    pub average_trade_rate_timeseries: Vec<TimeSeriesPoint>,
    pub energy_timeseries: Vec<EnergyTimeSeriesPoint>,
    pub total_matches: u64,
    pub success_rate: f64,
    /// Distinct buyers and sellers (together) over the matches in the window.
    pub active_traders: u64,
    /// Distinct markets in `market_data` in the window.
    pub markets: u64,
}

/// Totals over the `market_data` documents in a statistics window.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct MarketDataTotals {
    pub total_matches: u64,
    pub total_matched_energy_kWh: f64,
    pub total_unmatched_energy_kWh: f64,
    pub markets: u64,
}

/// Filter shared by all statistics and match queries: one user, optionally
/// one market, inclusive `[start_time, end_time]` on `time_slot`.
fn window_filter(
    user_id: String,
    market_id: Option<String>,
    start_time: u64,
    end_time: u64,
) -> Document {
    let mut filter = doc! {
        "time_slot": { "$gte": start_time as i64, "$lte": end_time as i64 },
        "user_id": user_id,
    };
    if let Some(market_id) = market_id {
        filter.insert("market_id", market_id);
    }
    filter
}

/// Aggregation expression giving the bucket of a document, in Unix seconds.
///
/// `NoAggregation` uses `time_slot` itself. The other resolutions truncate
/// `time_slot` with `$dateTrunc` (MongoDB 5.0+) in the time zone `tz` and
/// convert the start of the bucket back to Unix seconds.
fn bucket_expression(resolution: Resolution, tz: &str) -> Bson {
    let (unit, bin_size) = match resolution {
        Resolution::NoAggregation => return Bson::String("$time_slot".to_string()),
        Resolution::QuarterHour => ("minute", Some(15)),
        Resolution::Day => ("day", None),
        Resolution::Month => ("month", None),
    };
    let mut date_trunc = doc! {
        "date": { "$toDate": { "$multiply": ["$time_slot", 1000_i64] } },
        "unit": unit,
        "timezone": tz,
    };
    if let Some(bin_size) = bin_size {
        date_trunc.insert("binSize", bin_size);
    }
    Bson::Document(doc! {
        "$toLong": {
            "$divide": [{ "$toLong": { "$dateTrunc": date_trunc } }, 1000_i64]
        }
    })
}

/// Runs an aggregation and deserializes every resulting document.
async fn aggregate_into<T: DeserializeOwned>(
    collection: Collection<Document>,
    pipeline: Vec<Document>,
) -> mongodb::error::Result<Vec<T>> {
    let mut cursor = collection.aggregate(pipeline, None).await?;
    let mut results = Vec::new();
    while let Some(result) = cursor.next().await {
        results.push(mongodb::bson::from_document(result?)?);
    }
    Ok(results)
}

/// Indexes created by `ensure_indexes`: (collection, keys, unique).
fn index_specs() -> Vec<(&'static str, Document, bool)> {
    vec![
        // match_id last, so the /matches sort (time_slot, then match_id) is
        // served by the index.
        (
            "matches",
            doc! { "user_id": 1, "time_slot": 1, "match_id": 1 },
            false,
        ),
        (
            "matches",
            doc! { "user_id": 1, "market_id": 1, "time_slot": 1 },
            false,
        ),
        ("matches", doc! { "match_id": 1 }, true),
        (
            "market_data",
            doc! { "user_id": 1, "market_id": 1, "time_slot": 1 },
            true,
        ),
        ("market_data", doc! { "user_id": 1, "time_slot": 1 }, false),
    ]
}

/// Indexes of earlier versions that `ensure_indexes` drops: (collection,
/// index name). `matches.user_id_1_time_slot_1` is replaced by
/// `user_id_1_time_slot_1_match_id_1`.
const OBSOLETE_INDEXES: [(&str, &str); 1] = [("matches", "user_id_1_time_slot_1")];

/// MongoDB error codes meaning "nothing to drop": IndexNotFound and
/// NamespaceNotFound.
const INDEX_NOT_FOUND_CODES: [i32; 2] = [27, 26];

fn is_unreachable_error(e: &mongodb::error::Error) -> bool {
    matches!(
        e.kind.as_ref(),
        ErrorKind::ServerSelection { .. } | ErrorKind::Io(_)
    )
}

/// Creates the indexes of `matches` and `market_data` if they do not exist,
/// then drops obsolete indexes of earlier versions.
///
/// Idempotent. Each index is created on its own, so one failing index (for
/// example a unique index over old documents with duplicates) does not keep
/// the others from being created. Obsolete indexes are dropped only after
/// their replacements were attempted; a missing obsolete index is not an
/// error. Every failure is logged; the first one is returned. Stops early if
/// MongoDB cannot be reached, to avoid waiting for the server selection
/// timeout once per index.
pub async fn ensure_indexes(db: &Database) -> mongodb::error::Result<()> {
    let mut first_error = None;
    for (collection_name, keys, unique) in index_specs() {
        let options = IndexOptions::builder().unique(unique).build();
        let index = IndexModel::builder()
            .keys(keys.clone())
            .options(options)
            .build();
        let result = db
            .collection::<Document>(collection_name)
            .create_indexes(vec![index], None)
            .await;
        if let Err(e) = result {
            eprintln!(
                "Failed to create index {} (unique: {}) on collection {}: {:?}",
                keys, unique, collection_name, e
            );
            let unreachable = is_unreachable_error(&e);
            first_error.get_or_insert(e);
            if unreachable {
                return Err(first_error.unwrap());
            }
        }
    }
    for (collection_name, index_name) in OBSOLETE_INDEXES {
        let result = db
            .collection::<Document>(collection_name)
            .drop_index(index_name, None)
            .await;
        match result {
            Ok(()) => eprintln!(
                "Dropped obsolete index {} on collection {}",
                index_name, collection_name
            ),
            Err(e)
                if matches!(
                    e.kind.as_ref(),
                    ErrorKind::Command(c) if INDEX_NOT_FOUND_CODES.contains(&c.code)
                ) => {}
            Err(e) => {
                eprintln!(
                    "Failed to drop obsolete index {} on collection {}: {:?}",
                    index_name, collection_name, e
                );
                let unreachable = is_unreachable_error(&e);
                first_error.get_or_insert(e);
                if unreachable {
                    break;
                }
            }
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

pub struct MatchModel {
    pub db: mongodb::Database,
    pub collection_name: String,
}

impl MatchModel {
    /// Builds a model with its own `Client`. Used by tests and tools; the
    /// service itself uses `from_database` with the shared database handle.
    pub async fn new() -> mongodb::error::Result<Self> {
        let config = get_configuration().unwrap();
        let mongodb_uri = config.get_connection_string();
        let client = Client::with_uri_str(mongodb_uri).await?;
        let db = client.database(config.database_name.as_str());
        Ok(MatchModel::from_database(db))
    }

    /// Builds a model on an existing database handle (shared `Client`).
    pub fn from_database(db: mongodb::Database) -> Self {
        MatchModel {
            db,
            collection_name: "matches".to_string(),
        }
    }

    pub fn with_collection(mut self, name: &str) -> Self {
        self.collection_name = name.to_string();
        self
    }

    pub async fn insert_matches(
        &self,
        matches: Vec<DbBidOfferMatch>,
    ) -> mongodb::error::Result<()> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection(&self.collection_name);

        if !matches.is_empty() {
            collection.insert_many(matches, None).await?;
        }

        Ok(())
    }

    /// Average `energy_rate` per bucket over all trades in the bucket
    /// (a simple mean over trades, not a mean of per-slot means).
    pub async fn get_average_energy_rate_series(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        resolution: Resolution,
        tz: &str,
    ) -> mongodb::error::Result<Vec<TimeSeriesPoint>> {
        let pipeline = vec![
            doc! { "$match": window_filter(user_id, market_id, start_time, end_time) },
            doc! {
                "$group": {
                    "_id": bucket_expression(resolution, tz),
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
            doc! { "$sort": { "time_slot": 1 } },
        ];
        aggregate_into(self.db.collection(&self.collection_name), pipeline).await
    }

    /// Matched and unmatched energy per bucket, summed over `market_data`.
    pub async fn get_energy_series(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
        resolution: Resolution,
        tz: &str,
    ) -> mongodb::error::Result<Vec<EnergyTimeSeriesPoint>> {
        let pipeline = vec![
            doc! { "$match": window_filter(user_id, market_id, start_time, end_time) },
            doc! {
                "$group": {
                    "_id": bucket_expression(resolution, tz),
                    "matched_energy_kWh": { "$sum": "$total_matched_energy_kWh" },
                    "unmatched_energy_kWh": { "$sum": "$total_unmatched_energy_kWh" }
                }
            },
            doc! {
                "$project": {
                    "time_slot": "$_id",
                    "matched_energy_kWh": 1,
                    "unmatched_energy_kWh": 1,
                    "_id": 0
                }
            },
            doc! { "$sort": { "time_slot": 1 } },
        ];
        aggregate_into(self.db.collection("market_data"), pipeline).await
    }

    /// Totals over `market_data` in the window: matches, matched and
    /// unmatched energy, and the number of distinct markets.
    pub async fn get_market_data_totals(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
    ) -> mongodb::error::Result<MarketDataTotals> {
        let pipeline = vec![
            doc! { "$match": window_filter(user_id, market_id, start_time, end_time) },
            doc! {
                "$group": {
                    "_id": Bson::Null,
                    "total_matches": { "$sum": "$total_matches" },
                    "total_matched_energy_kWh": { "$sum": "$total_matched_energy_kWh" },
                    "total_unmatched_energy_kWh": { "$sum": "$total_unmatched_energy_kWh" },
                    "markets": { "$addToSet": "$market_id" }
                }
            },
            doc! {
                "$project": {
                    "_id": 0,
                    "total_matches": 1,
                    "total_matched_energy_kWh": 1,
                    "total_unmatched_energy_kWh": 1,
                    "markets": { "$size": "$markets" }
                }
            },
        ];
        let totals: Vec<MarketDataTotals> =
            aggregate_into(self.db.collection("market_data"), pipeline).await?;
        Ok(totals.into_iter().next().unwrap_or_default())
    }

    /// Number of distinct traders over the matches in the window: the values
    /// of `bid.buyer` and `offer.seller` together, so an account that both
    /// bought and sold counts once.
    pub async fn count_active_traders(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
    ) -> mongodb::error::Result<u64> {
        #[derive(Deserialize)]
        struct Count {
            count: u64,
        }

        let pipeline = vec![
            doc! { "$match": window_filter(user_id, market_id, start_time, end_time) },
            doc! { "$project": { "_id": 0, "traders": ["$bid.buyer", "$offer.seller"] } },
            doc! { "$unwind": "$traders" },
            doc! { "$group": { "_id": "$traders" } },
            doc! { "$count": "count" },
        ];
        let counts: Vec<Count> =
            aggregate_into(self.db.collection(&self.collection_name), pipeline).await?;
        Ok(counts.first().map_or(0, |c| c.count))
    }

    /// Matches in the window, sorted by `time_slot` and then `match_id`, both
    /// in the direction of `sort`, so `offset` / `limit` pages are stable.
    #[allow(clippy::too_many_arguments)]
    pub async fn get_matches(
        &self,
        start_time: u64,
        end_time: u64,
        user_id: String,
        market_id: Option<String>,
        limit: Option<i64>,
        offset: Option<u64>,
        sort: SortOrder,
    ) -> mongodb::error::Result<Vec<DbBidOfferMatch>> {
        let collection: Collection<DbBidOfferMatch> = self.db.collection(&self.collection_name);

        let filter = window_filter(user_id, market_id, start_time, end_time);
        let direction = sort.direction();
        let options = FindOptions::builder()
            .sort(doc! { "time_slot": direction, "match_id": direction })
            .limit(limit)
            .skip(offset)
            // Unpaged requests over large windows may need a sort larger
            // than the in-memory limit.
            .allow_disk_use(true)
            .build();

        let mut cursor = collection.find(filter, options).await?;
        let mut results = Vec::new();

        while let Some(result) = cursor.next().await {
            results.push(result?);
        }

        Ok(results)
    }

    pub async fn upsert_market_data(
        &self,
        market_data_list: Vec<DbMarketData>,
    ) -> mongodb::error::Result<()> {
        let collection: Collection<DbMarketData> = self.db.collection("market_data");

        for data in market_data_list {
            let filter = doc! {
                "user_id": &data.user_id,
                "market_id": &data.market_id,
                "time_slot": data.time_slot as i64
            };

            let update = doc! {
                "$inc": {
                    "submitted_bid_count": data.submitted_bid_count as i64,
                    "submitted_offer_count": data.submitted_offer_count as i64,
                    "total_matches": data.total_matches as i64,
                    "total_matched_energy_kWh": data.total_matched_energy_kWh,
                    "total_unmatched_energy_kWh": data.total_unmatched_energy_kWh
                }
            };

            let options = UpdateOptions::builder().upsert(true).build();
            collection.update_one(filter, update, options).await?;
        }

        Ok(())
    }

    pub async fn get_market_data(
        &self,
        user_id: String,
        market_id: Option<String>,
        start_time: u64,
        end_time: u64,
    ) -> mongodb::error::Result<Vec<DbMarketData>> {
        let collection: Collection<DbMarketData> = self.db.collection("market_data");

        let mut filter = doc! {
            "time_slot": { "$gte": start_time as i64, "$lte": end_time as i64 },
            "user_id": user_id
        };

        if let Some(market_id) = market_id {
            filter.insert("market_id", market_id);
        }

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "time_slot": 1 })
            .build();

        let mut cursor = collection.find(filter, options).await?;
        let mut results = Vec::new();

        while let Some(result) = cursor.next().await {
            results.push(result?);
        }

        Ok(results)
    }

    pub async fn get_markets(&self, user_id: String) -> mongodb::error::Result<Vec<String>> {
        let collection: Collection<DbMarketData> = self.db.collection("market_data");
        let filter = doc! { "user_id": user_id };
        let cursor = collection.distinct("market_id", filter, None).await?;
        let mut results = Vec::new();
        for val in cursor {
            if let Some(market_id) = val.as_str() {
                results.push(market_id.to_string());
            }
        }
        Ok(results)
    }
}
