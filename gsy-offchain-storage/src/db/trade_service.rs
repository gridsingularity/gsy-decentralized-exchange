use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, UpdateSummary, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use mongodb::bson;
use mongodb::bson::{Bson, doc};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// this struct is wrapper to `Collection<Trade>` should have function to help to manage order
pub struct TradeService(pub Coll<TradeSchema>);

impl TradeService {
    #[tracing::instrument(name = "Fetching trades from database", skip(self))]
    pub async fn get_all_trades(&self) -> Result<Vec<TradeSchema>> {
        self.0.all().await
    }

    #[tracing::instrument(
        name = "Saving trades to database",
        skip(self, trade_schema),
        fields(
            trade_schema = ?trade_schema
        )
    )]
    /// Store each trade unless one with its `trade_uuid` is already stored, and return the
    /// `_id`s of the newly stored ones by their index in `trade_schema`.
    ///
    /// The orderbook worker re-posts a trade whose earlier post it could not clear from its queue,
    /// and every post carries a fresh random `_id`. Keying on `trade_uuid` keeps one record per
    /// trade. A re-post leaves the stored record as it is, status included, so it cannot reset a
    /// verdict back to `Settled`; and it still succeeds, so the worker clears the trade.
    pub async fn insert_trades(
        &self,
        trade_schema: Vec<TradeSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        let mut inserted_ids = HashMap::new();
        for (index, trade) in trade_schema.into_iter().enumerate() {
            let trade_uuid = trade.trade_uuid.clone();
            let id = Bson::String(trade._id.clone());
            let inserted = self
                .0
                .insert_if_absent(doc! {"trade_uuid": &trade_uuid}, trade, |stored| {
                    stored.trade_uuid == trade_uuid
                })
                .await?;
            if inserted {
                inserted_ids.insert(index, id);
            }
        }
        Ok(inserted_ids)
    }

    /// Delete all but one record of every `trade_uuid` stored more than once, and return the
    /// number of records deleted. Of each group the kept record is, in order of preference:
    /// one with a verdict (`Executed`/`Penalized`) over one still `Settled`; then the oldest,
    /// by `creation_time` and then by `status_updated_at` (the earliest verdict; a missing one
    /// sorts first); then the smallest `_id`. Copies of one trade share `creation_time`, as it is
    /// set on-chain, and `_id` is a random uuid, so between two `Settled` copies (identical apart
    /// from `_id`) the last rule just makes the choice deterministic.
    pub async fn remove_duplicate_trades(&self) -> Result<u64> {
        let settled = bson::to_bson(&TradeStatus::Settled)?;
        self.0
            .remove_duplicates_by(
                &["trade_uuid"],
                vec![
                    doc! {"$addFields": {
                        "_unjudged": {"$cond": [{"$eq": ["$status", settled]}, 1, 0]},
                    }},
                    doc! {"$sort": {
                        "_unjudged": 1,
                        "creation_time": 1,
                        "status_updated_at": 1,
                        "_id": 1,
                    }},
                ],
                |trade| trade.trade_uuid.clone(),
                |a, b| {
                    let unjudged = |trade: &TradeSchema| trade.status == TradeStatus::Settled;
                    unjudged(a)
                        .cmp(&unjudged(b))
                        .then(a.creation_time.cmp(&b.creation_time))
                        .then(a.status_updated_at.cmp(&b.status_updated_at))
                        .then(a._id.cmp(&b._id))
                },
            )
            .await
    }

    #[tracing::instrument(name = "Fetching trades by market id from database", skip(self))]
    pub async fn filter_trades(
        &self,
        market_id: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
        status: Option<TradeStatus>,
    ) -> Result<Vec<TradeSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = &market_id {
            filter_params.insert("market_id", market_id.clone());
        }
        if let Some(status) = &status {
            filter_params.insert("status", bson::to_bson(status)?);
        }
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |trade| {
                market_id
                    .as_ref()
                    .is_none_or(|market_id| &trade.market_id == market_id)
                    && status.as_ref().is_none_or(|status| &trade.status == status)
                    && in_time_window(trade.time_slot, start_time, end_time)
            })
            .await
    }

    /// Fetch trades whose **status last changed** within `[start_time, end_time]` (both
    /// inclusive, unix seconds), optionally restricted to one status.
    ///
    /// This windows on `status_updated_at` rather than `time_slot`: the delivery slot says
    /// when energy flowed, whereas this says when the trade reached its verdict. `Executed`
    /// and `Penalized` are terminal, so for those the value never moves again once set.
    ///
    /// Trades with no `status_updated_at` are excluded. That is every trade written before
    /// the field existed, and every trade still sitting in `Settled` — neither has a status
    /// change to window on.
    #[tracing::instrument(name = "Fetching trades by status change time", skip(self))]
    pub async fn filter_trades_by_status_change(
        &self,
        start_time: u64,
        end_time: Option<u64>,
        status: Option<TradeStatus>,
    ) -> Result<Vec<TradeSchema>> {
        let mut bounds = doc! {"$gte": bson::to_bson(&start_time)?};
        if let Some(end_time) = end_time {
            bounds.insert("$lte", bson::to_bson(&end_time)?);
        }
        let mut filter_params = doc! {"status_updated_at": bounds};
        if let Some(status) = &status {
            filter_params.insert("status", bson::to_bson(status)?);
        }

        self.0
            .query(filter_params, |trade| {
                status.as_ref().is_none_or(|status| &trade.status == status)
                    && trade.status_updated_at.is_some_and(|changed_at| {
                        changed_at >= start_time
                            && end_time.is_none_or(|end_time| changed_at <= end_time)
                    })
            })
            .await
    }

    #[tracing::instrument(name = "Fetching trades by area uuid from database", skip(self))]
    pub async fn get_trades_by_area(
        &self,
        area_uuid: String,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<TradeSchema>> {
        // The area participates in a trade on either the bid or the offer side,
        // so match its area_uuid under both nested component paths with `$or`.
        let mut filter_params = doc! {"$or": [
            { "bid.bid_component.area_uuid": &area_uuid },
            { "offer.offer_component.area_uuid": &area_uuid }
        ]};
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |trade| {
                (trade.bid.bid_component.area_uuid == area_uuid
                    || trade.offer.offer_component.area_uuid == area_uuid)
                    && in_time_window(trade.time_slot, start_time, end_time)
            })
            .await
    }

    /// Set the status of every record of `trade_uuid` not already in `status`. There is one
    /// record per trade once the start-up dedupe has run, but all copies are updated, so a trade
    /// stored twice cannot keep a copy behind in `Settled` that would be judged again.
    #[tracing::instrument(
        name = "Update trade status by trade_uuid",
        skip(self, trade_uuid, status)
    )]
    pub async fn update_trade_status_by_uuid(
        &self,
        trade_uuid: &str,
        status: TradeStatus,
    ) -> Result<UpdateSummary> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.0
            .update_many(
                doc! {
                    "trade_uuid": trade_uuid,
                    "status": {"$ne": bson::to_bson(&status)?}
                },
                doc! {
                    "$set": {
                        "status": bson::to_bson(&status)?,
                        "status_updated_at": bson::to_bson(&now)?,
                    }
                },
                |trade| trade.trade_uuid == trade_uuid && trade.status != status,
                |trade| {
                    trade.status = status.clone();
                    trade.status_updated_at = Some(now);
                    true
                },
            )
            .await
    }
}

impl From<&DatabaseWrapper> for TradeService {
    fn from(db: &DatabaseWrapper) -> Self {
        TradeService(db.coll("trades", |store| store.trades.clone()))
    }
}
