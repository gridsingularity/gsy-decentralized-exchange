use crate::db::DbRef;
use crate::routes::validate_start_end_time;
use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use mongodb::bson::Bson;
use primitives::db_api_schema::orders::OrderStatus;
use primitives::db_api_schema::trades::TradeSchema;
use serde::Deserialize;

#[tracing::instrument(name = "Adding new trades", skip(db), fields(trades = ?trades))]
pub async fn post_trades(trades: Json<Vec<TradeSchema>>, db: DbRef) -> impl Responder {
    for trade in trades.iter() {
        let bid_id = Bson::String(trade.bid_hash.clone());
        let offer_id = Bson::String(trade.offer_hash.clone());
        let _ = db
            .get_ref()
            .orders()
            .update_order_status_by_id(&bid_id, OrderStatus::Executed)
            .await;
        let _ = db
            .get_ref()
            .orders()
            .update_order_status_by_id(&offer_id, OrderStatus::Executed)
            .await;
    }

    match db.get_ref().trades().insert_trades(trades.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_trades(trades: Json<Vec<TradeSchema>>, db: DbRef) -> impl Responder {
    post_trades(trades, db).await
}

#[derive(Deserialize, Debug)]
pub struct GetTradesParams {
    start_time: Option<u64>,
    end_time: Option<u64>,
}

#[tracing::instrument(name = "Retrieve trades", skip(db))]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    if let Err(response) = validate_start_end_time(query_params.start_time, query_params.end_time) {
        return response;
    }
    match db
        .get_ref()
        .trades()
        .filter_trades(query_params.start_time, query_params.end_time)
        .await
    {
        Ok(trades) => HttpResponse::Ok().json(trades),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
