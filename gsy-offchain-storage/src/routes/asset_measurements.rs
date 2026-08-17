use crate::certificates::builder::build_local_origin_records;
use crate::certificates::schema::LocalOriginRecord;
use crate::db::DbRef;
use actix_web::{HttpResponse, Responder, web::Query};
use gsy_offchain_primitives::db_api_schema::trades::TradeStatus;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct GuaranteesOfOriginParams {
    /// Lower bound (inclusive) on when the trade reached `Executed`, in unix seconds.
    /// Mandatory: an absent bound would scan every trade ever settled.
    start_time: u64,
    /// Upper bound (inclusive) on when the trade reached `Executed`. Absent means "up to now".
    end_time: Option<u64>,
}

/// Derive Annex A `local_origin_record` certificates from `Executed` trades.
///
/// Only `Executed` trades qualify: the execution engine compared them against metering and
/// they incurred no penalty, so the exchange can attest to the traded quantity. The unit of
/// issuance is the trade, not the metered volume — these certify *traded* energy.
///
/// The window bounds **when the trade was validated** (`status_updated_at`), not when the
/// energy flowed. `Executed` is terminal, so that timestamp never moves again once set, which
/// makes it a stable thing to window on: metering arrives after the interval it describes and
/// by a variable delay, so a window over delivery time would advance past slots that are
/// still awaiting their verdict.
#[tracing::instrument(name = "Derive guarantees of origin from executed trades", skip(db))]
pub async fn get_guarantees_of_origin(
    db: DbRef,
    query_params: Query<GuaranteesOfOriginParams>,
) -> impl Responder {
    let trades = match db
        .get_ref()
        .trades()
        .filter_trades_by_status_change(
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

    if trades.is_empty() {
        return HttpResponse::Ok().json(Vec::<LocalOriginRecord>::new());
    }

    let markets = match db.get_ref().markets().all_markets().await {
        Ok(markets) => markets,
        Err(e) => {
            tracing::error!("Failed to fetch markets: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // A measurement's `time_slot` is a delivery slot, so the status-change window above says
    // nothing about which measurements are needed. Bound the fetch by the delivery slots of
    // the trades actually selected. `u32::try_from` failing widens the bound to unbounded
    // rather than truncating: a superset is always correct here, a truncated bound is not.
    let (earliest_slot, latest_slot) = trades.iter().fold((u64::MAX, 0), |(min, max), trade| {
        (min.min(trade.time_slot), max.max(trade.time_slot))
    });

    let measurements = match db
        .get_ref()
        .measurements()
        .filter_measurements(
            None,
            u32::try_from(earliest_slot).ok(),
            u32::try_from(latest_slot).ok(),
        )
        .await
    {
        Ok(measurements) => measurements,
        Err(e) => {
            tracing::error!("Failed to fetch measurements: {:?}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut records = build_local_origin_records(trades, &markets, &measurements);

    // Deterministic order so repeated or adjacent queries return a stable sequence.
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
