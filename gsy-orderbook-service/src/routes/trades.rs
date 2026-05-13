use crate::db::DbRef;
use actix_web::web::Query;
use actix_web::{web::Json, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;
use gsy_offchain_primitives::node_to_api_schema::insert_trades::convert_gsy_node_trades_schema_to_db_schema;
use serde::Deserialize;

#[tracing::instrument(
    name = "Adding new trades",
    skip(trades, db),
    fields(trades = ?trades)
)]
pub async fn post_trades(trades: Json<Vec<u8>>, db: DbRef) -> impl Responder {
    let deserialized_trades = convert_gsy_node_trades_schema_to_db_schema(trades.to_vec());

    // Each trade settles its bid and offer in the Order Book — surface the
    // executed status by updating both orders by id. The matching engine
    // uses `bid_id` / `offer_id` to reference the underlying orders.
    for trade in deserialized_trades.iter() {
        let _ = db
            .get_ref()
            .orders()
            .update_order_status_by_id(
                &trade.bid_id,
                gsy_offchain_primitives::db_api_schema::orders::OrderStatus::Executed,
            )
            .await;
        let _ = db
            .get_ref()
            .orders()
            .update_order_status_by_id(
                &trade.offer_id,
                gsy_offchain_primitives::db_api_schema::orders::OrderStatus::Executed,
            )
            .await;
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
    start_time: Option<String>,
    end_time: Option<String>,
}

#[tracing::instrument(name = "Retrieve trades", skip(db))]
pub async fn get_trades(db: DbRef, query_params: Query<GetTradesParams>) -> impl Responder {
    match db
        .get_ref()
        .trades()
        .filter_trades(
            query_params.start_time.clone(),
            query_params.end_time.clone(),
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
