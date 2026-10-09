use crate::db::DbRef;
use crate::routes::parse_time_range;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use anyhow::{Error, Result};
use primitives::db_api_schema::orders::{DbOrderSchema, FlexibilityOrderSchema};
use primitives::ewds::dto::EwdsOrderDto;
use serde::Deserialize;

pub async fn post_orders(orders: Json<Vec<EwdsOrderDto>>, db: DbRef) -> impl Responder {
    let db_orders: Result<Vec<DbOrderSchema>, _> = orders
        .into_inner()
        .into_iter()
        .map(DbOrderSchema::try_from)
        .collect();

    let db_orders = match db_orders {
        Ok(o) => o,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    match db.get_ref().orders().insert_orders(db_orders).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn post_normalized_orders(orders: Json<Vec<EwdsOrderDto>>, db: DbRef) -> impl Responder {
    post_orders(orders, db).await
}

#[derive(Deserialize)]
pub struct OrdersParameters {
    #[serde(default)]
    market_id: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
}

async fn filter_orders_from_db(
    db: DbRef,
    market_id: Option<String>,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<Vec<EwdsOrderDto>, Error> {
    let orders = if market_id.is_none() && start_time.is_none() && end_time.is_none() {
        db.get_ref().orders().get_all_orders().await?
    } else {
        db.get_ref()
            .orders()
            .filter_orders(market_id, start_time, end_time)
            .await?
    };
    Ok(orders.into_iter().map(EwdsOrderDto::from).collect())
}

pub async fn get_orders(db: DbRef, orders_parameters: Query<OrdersParameters>) -> impl Responder {
    let (start_time, end_time) = match parse_time_range(
        orders_parameters.start_time.as_deref(),
        orders_parameters.end_time.as_deref(),
    ) {
        Ok(range) => range,
        Err(response) => return response,
    };

    let market_id = orders_parameters.into_inner().market_id;
    match filter_orders_from_db(db, market_id, start_time, end_time).await {
        Ok(orders) => HttpResponse::Ok().json(
            orders
                .into_iter()
                .map(EwdsOrderDto::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_flexibility_orders(
    orders: Json<Vec<FlexibilityOrderSchema>>,
    db: DbRef,
) -> impl Responder {
    match db
        .get_ref()
        .flexibility_orders()
        .insert_orders(orders.to_vec())
        .await
    {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_flexibility_orders(db: DbRef) -> impl Responder {
    match db.get_ref().flexibility_orders().get_all_orders().await {
        Ok(orders) => HttpResponse::Ok().json(orders),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
