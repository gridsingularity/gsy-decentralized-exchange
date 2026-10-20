use super::{KpiQuery, ResultsRef};
use crate::db::results::find_results;
use actix_web::web::Query;
use actix_web::{HttpResponse, Responder};
use primitives::db_api_schema::kpi::{
    ProcurementCostResultSchema, PROCUREMENT_COST_PER_KWH_KPI_ID,
};

/// `GET /kpis/procurement-cost-per-kwh?start_time=&end_time=&community_id=`
pub async fn get_procurement_cost_per_kwh(
    results: ResultsRef,
    query: Query<KpiQuery>,
) -> impl Responder {
    let filter = match query.to_filter(PROCUREMENT_COST_PER_KWH_KPI_ID) {
        Ok(filter) => filter,
        Err(response) => return response,
    };
    match find_results::<ProcurementCostResultSchema>(results.get_ref(), &filter).await {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(error) => {
            tracing::error!(
                "Failed to read procurement cost per kWh results: {:?}",
                error
            );
            HttpResponse::InternalServerError().finish()
        }
    }
}
