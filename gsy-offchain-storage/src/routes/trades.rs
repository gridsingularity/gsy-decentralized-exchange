use crate::db::DbRef;
use actix_web::web::Query;
use actix_web::{HttpResponse, Responder, web::Json};
use gsy_offchain_primitives::db_api_schema::trades::{TradeCanonicalSchema, TradeSchema};
use gsy_offchain_primitives::node_to_api_schema::insert_trades::convert_gsy_node_trades_schema_to_db_schema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[tracing::instrument(
    name = "Adding new trades",
    skip(trades, db),
    fields(
    trades = ?trades
    )
)]
pub async fn post_trades(trades: Json<Vec<u8>>, db: DbRef) -> impl Responder {
    let deserialized_trades = convert_gsy_node_trades_schema_to_db_schema(trades.to_vec());
    for trade in deserialized_trades.clone() {
        let orders = db.get_ref().orders();
        if let Err(e) = orders
            .update_order_by_area_market_id(
                trade.offer.offer_component.area_uuid.clone(),
                trade.market_id.clone(),
            )
            .await
        {
            tracing::error!(
                "Failed to mark offer order Executed (area {}, market {}): {:?}",
                trade.offer.offer_component.area_uuid,
                trade.market_id,
                e
            );
        }
        if let Err(e) = orders
            .update_order_by_area_market_id(
                trade.bid.bid_component.area_uuid.clone(),
                trade.market_id.clone(),
            )
            .await
        {
            tracing::error!(
                "Failed to mark bid order Executed (area {}, market {}): {:?}",
                trade.bid.bid_component.area_uuid,
                trade.market_id,
                e
            );
        }
    }
    match db
        .get_ref()
        .trades()
        .insert_trades(deserialized_trades)
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_trades(trades: Json<Vec<TradeSchema>>, db: DbRef) -> impl Responder {
    match db.get_ref().trades().insert_trades(trades.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    market_id: Option<String>,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

#[tracing::instrument(name = "Retrieve trades", skip(db))]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    match db
        .get_ref()
        .trades()
        .filter_trades(
            query_params.market_id.clone(),
            query_params.start_time,
            query_params.end_time,
        )
        .await
    {
        Ok(trades) => HttpResponse::Ok().json(trades),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(name = "Retrieve canonical trades with resolved asset names", skip(db))]
pub async fn get_trades_canonical(
    db: DbRef,
    query_params: Query<GetTradesParams>,
) -> impl Responder {
    let trades = match db
        .get_ref()
        .trades()
        .filter_trades(
            query_params.market_id.clone(),
            query_params.start_time,
            query_params.end_time,
        )
        .await
    {
        Ok(trades) => trades,
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let markets = match db.get_ref().markets().all_markets().await {
        Ok(markets) => markets,
        Err(e) => {
            tracing::error!("Failed to fetch markets: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Build a global area_hash -> name map from every market's topology.
    // area_hash is globally unique (randomized per market) so this is
    // unambiguous even across markets.
    let mut name_by_area_hash: HashMap<String, String> = HashMap::new();
    for market in &markets {
        for area in &market.community_areas {
            name_by_area_hash.insert(area.area_hash.clone(), area.name.clone());
        }
    }

    // Each trade component's `area_uuid` holds the asset's `area_hash`.
    let canonical: Vec<TradeCanonicalSchema> = trades
        .into_iter()
        .map(|trade| {
            let seller_name = name_by_area_hash
                .get(&trade.offer.offer_component.area_uuid)
                .cloned();
            let buyer_name = name_by_area_hash
                .get(&trade.bid.bid_component.area_uuid)
                .cloned();
            TradeCanonicalSchema {
                trade,
                seller_name,
                buyer_name,
            }
        })
        .collect();

    HttpResponse::Ok().json(canonical)
}

#[derive(Deserialize, Debug)]
pub struct GetTradedEnergyParams {
    id: String,
    start_time: Option<u32>,
    end_time: Option<u32>,
}

#[derive(Serialize, Debug)]
pub struct TimeSeriesPoint {
    pub timestamp: u64,
    pub value: f64,
}

#[derive(Serialize, Debug)]
pub struct TradedEnergyResponse {
    pub id: String,
    pub traded_energy: Vec<TimeSeriesPoint>,
}

#[tracing::instrument(name = "Retrieve traded energy for area", skip(db))]
pub async fn get_traded_energy(
    db: DbRef,
    query_params: Query<GetTradedEnergyParams>,
) -> impl Responder {
    match db
        .get_ref()
        .trades()
        .get_trades_by_area(
            query_params.id.clone(),
            query_params.start_time,
            query_params.end_time,
        )
        .await
    {
        Ok(trades) => {
            // Sum the selected energy of all matching trades per time_slot;
            // BTreeMap keeps the series sorted ascending by timestamp.
            let mut energy_per_time_slot: BTreeMap<u64, f64> = BTreeMap::new();
            for trade in trades {
                *energy_per_time_slot.entry(trade.time_slot).or_insert(0.0) +=
                    trade.parameters.selected_energy;
            }
            let traded_energy = energy_per_time_slot
                .into_iter()
                .map(|(timestamp, value)| TimeSeriesPoint { timestamp, value })
                .collect();
            HttpResponse::Ok().json(TradedEnergyResponse {
                id: query_params.id.clone(),
                traded_energy,
            })
        }
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
