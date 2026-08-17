use crate::certificates::builder::build_local_origin_records;
use crate::certificates::schema::LocalOriginRecord;
use crate::db::DbRef;
use actix_web::{HttpResponse, Responder, web::Query};
use gsy_offchain_primitives::db_api_schema::trades::TradeStatus;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GuaranteesOfOriginParams {
    /// Lower bound on the delivery interval (`interval_start`), inclusive.
    start_time: Option<u32>,
    /// Upper bound on the delivery interval (`interval_start`), inclusive.
    end_time: Option<u32>,
    /// Lower bound on `measurement_recorded_at`, **exclusive** — the checkpoint to poll on.
    recorded_after: Option<u64>,
    /// Upper bound on `measurement_recorded_at`, inclusive.
    recorded_before: Option<u64>,
}

/// Derive Annex A `local_origin_record` certificates from `Executed` trades.
///
/// Only `Executed` trades qualify: the execution engine compared them against metering and
/// they incurred no penalty, so the exchange can attest to the traded quantity. The unit of
/// issuance is the trade, not the metered volume — these certify *traded* energy.
///
/// `recorded_after`/`recorded_before` are applied in-process rather than in the Mongo query
/// because `measurement_recorded_at` is a max across two collections, which no single-document
/// predicate expresses. At pilot volume the scan is negligible, so no new index.
#[tracing::instrument(name = "Derive guarantees of origin from executed trades", skip(db))]
pub async fn get_guarantees_of_origin(
    db: DbRef,
    query_params: Query<GuaranteesOfOriginParams>,
) -> impl Responder {
    let trades = match db
        .get_ref()
        .trades()
        .filter_trades(
            None,
            query_params.start_time,
            query_params.end_time,
            Some(TradeStatus::Executed),
        )
        .await
    {
        Ok(trades) => trades,
        Err(e) => {
            tracing::error!("Failed to fetch trades: {:?}", e);
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

    let measurements = match db
        .get_ref()
        .measurements()
        .filter_measurements(None, query_params.start_time, query_params.end_time)
        .await
    {
        Ok(measurements) => measurements,
        Err(e) => {
            tracing::error!("Failed to fetch measurements: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut records = build_local_origin_records(trades, &markets, &measurements);

    records.retain(|record| {
        let recorded_at = record.measurement_provenance.measurement_recorded_at;
        query_params
            .recorded_after
            .is_none_or(|after| recorded_at > after)
            && query_params
                .recorded_before
                .is_none_or(|before| recorded_at <= before)
    });

    // Deterministic order so a consumer paging on `recorded_after` sees a stable sequence.
    records.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    HttpResponse::Ok().json(records)
}

fn sort_key(record: &LocalOriginRecord) -> (u64, u64, &str, &str) {
    (
        record.measurement_provenance.measurement_recorded_at,
        record.time_and_quantity.source_slot_timestamp,
        record.production_asset.production_asset_id.as_str(),
        record
            .trade_and_delivery
            .trade_reference
            .first()
            .map(String::as_str)
            .unwrap_or_default(),
    )
}
