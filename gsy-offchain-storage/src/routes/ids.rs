use crate::db::DbRef;
use actix_web::web::Query;
use actix_web::{HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct IdsParams {
    offchain_id: String,
}

#[tracing::instrument(name = "get or create ids", skip(db))]
pub async fn get_or_create_ids(db: DbRef, query_params: Query<IdsParams>) -> impl Responder {
    let offchain_id = match primitives::utils::normalize_offchain_id(&query_params.offchain_id) {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    match db.get_ref().ids().get_or_create(offchain_id).await {
        Ok(id) => HttpResponse::Ok().json(id),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
