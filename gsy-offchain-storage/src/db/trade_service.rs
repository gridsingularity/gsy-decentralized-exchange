use crate::db::DatabaseWrapper;
use crate::db::collection::{Coll, apply_time_window, in_time_window};
use anyhow::Result;
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use mongodb::bson::{Bson, doc};
use std::collections::HashMap;

/// this struct is wrapper to `Collection<Trade>` should have function to help to manage order
pub struct TradeService(pub(crate) Coll<TradeSchema>);

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
    pub async fn insert_trades(
        &self,
        trade_schema: Vec<TradeSchema>,
    ) -> Result<HashMap<usize, Bson>> {
        self.0
            .insert_many(trade_schema, |trade| Bson::String(trade._id.clone()))
            .await
    }

    #[tracing::instrument(name = "Fetching trades by market id from database", skip(self))]
    pub async fn filter_trades(
        &self,
        market_id: Option<String>,
        start_time: Option<u32>,
        end_time: Option<u32>,
    ) -> Result<Vec<TradeSchema>> {
        let mut filter_params = doc! {};
        if let Some(market_id) = &market_id {
            filter_params.insert("market_id", market_id.clone());
        }
        apply_time_window(&mut filter_params, start_time, end_time);

        self.0
            .query(filter_params, |trade| {
                market_id
                    .as_ref()
                    .is_none_or(|market_id| &trade.market_id == market_id)
                    && in_time_window(trade.time_slot, start_time, end_time)
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
}

impl From<&DatabaseWrapper> for TradeService {
    fn from(db: &DatabaseWrapper) -> Self {
        TradeService(db.coll("trades", |store| store.trades.clone()))
    }
}
