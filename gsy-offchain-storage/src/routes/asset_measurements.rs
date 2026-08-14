use crate::db::DbRef;
use actix_web::{HttpResponse, Responder, web::Query};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct GuaranteesOfOriginParams {
    start_time: Option<u32>,
    end_time: Option<u32>,
}

/// A single settled energy trade, shaped for guarantees-of-origin reporting.
///
/// In FEDECOM the energy is traded ahead of delivery: matching happens against
/// forecasts, so `energy_trade_timestamp` (when the trade was struck) precedes
/// `energy_delivery_timestamp` (the delivery time slot the energy is settled
/// for).
#[derive(Serialize, Debug)]
pub struct GuaranteesOfOriginSchema {
    /// Unique id of the energy trade.
    pub trade_id: String,
    /// Traded energy in kWh.
    pub traded_energy_kwh: f64,
    /// Account id of the buyer.
    pub buyer_id: String,
    /// Account id of the seller.
    pub seller_id: String,
    /// Energy community the traded assets belong to.
    pub energy_community_id: String,
    /// Delivery time of the traded energy (internally the market `time_slot`).
    pub energy_delivery_timestamp: u64,
    /// Time at which the energy was traded. Precedes the delivery timestamp
    /// because trading is driven by forecasts rather than measurements.
    pub energy_trade_timestamp: u64,
    /// Market the energy was traded in.
    pub market_id: String,
    /// Market type the energy was traded in (e.g. "Spot", "Flexibility").
    pub market_type: String,
}

#[tracing::instrument(
    name = "Retrieve settled trades for guarantees of origin",
    skip(db)
)]
pub async fn get_guarantees_of_origin(
    db: DbRef,
    query_params: Query<GuaranteesOfOriginParams>,
) -> impl Responder {
    let trades = match db
        .get_ref()
        .trades()
        // Status is deliberately left unfiltered to preserve the existing behaviour; whether a
        // penalized trade should still earn a guarantee of origin is an open question.
        .filter_trades(None, query_params.start_time, query_params.end_time, None)
        .await
    {
        Ok(trades) => trades,
        Err(e) => {
            tracing::error!("Failed to fetch trades: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Resolve each trade's energy community from its market topology. market_id
    // is globally unique, so a single market_id -> community_uuid map suffices.
    let markets = match db.get_ref().markets().all_markets().await {
        Ok(markets) => markets,
        Err(e) => {
            tracing::error!("Failed to fetch markets: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };
    let community_by_market: HashMap<String, String> = markets
        .into_iter()
        .map(|market| (market.market_id, market.community_uuid))
        .collect();

    let guarantees: Vec<GuaranteesOfOriginSchema> = trades
        .into_iter()
        .map(|trade| {
            let energy_community_id = community_by_market
                .get(&trade.market_id)
                .cloned()
                .unwrap_or_default();
            GuaranteesOfOriginSchema {
                trade_id: trade.trade_uuid,
                traded_energy_kwh: trade.parameters.selected_energy,
                buyer_id: trade.buyer,
                seller_id: trade.seller,
                energy_community_id,
                energy_delivery_timestamp: trade.time_slot,
                energy_trade_timestamp: trade.creation_time,
                market_id: trade.market_id,
                // Only Spot markets are produced today; the market type is not
                // yet persisted, so it is reported as a constant for now.
                market_type: "Spot".to_string(),
            }
        })
        .collect();

    HttpResponse::Ok().json(guarantees)
}
