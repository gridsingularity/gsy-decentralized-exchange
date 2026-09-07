use crate::db::DbRef;
use actix_web::web::Query;
use serde::Deserialize;
use actix_web::{HttpResponse, Responder};

#[derive(Deserialize, Debug)]
pub struct IdsParams {
    offchain_id: String,
}


#[tracing::instrument(name = "get or create ids", skip(db))]
pub async fn get_or_create_ids(db: DbRef, query_params: Query<IdsParams>) -> impl Responder {
    match db
        .get_ref()
        .ids()
        .get_or_create(query_params.offchain_id.clone())
        .await
    {
        Ok(id) => {
            HttpResponse::Ok().json(id)
        }
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
