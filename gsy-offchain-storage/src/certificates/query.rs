//! The guarantees-of-origin query shared by REST (`GET /guarantees-of-origin-measurements`)
//! and EWDS (`guarantees_of_origin.query`): window validation, the data fetch, and the
//! deterministic ordering around the pure builder.

use anyhow::Result;
use primitives::certificates::LocalOriginRecord;
use primitives::db_api_schema::profiles::MeasurementPointType;
use primitives::db_api_schema::trades::TradeStatus;
use primitives::utils::timestamp_to_string_with_padding;

use crate::certificates::builder::{
    build_local_origin_records_with_allocation, sort_records, CertificateInputs,
};
use crate::db::DatabaseWrapper;

/// Maximum width of a query window, `end_time - start_time`, in seconds (D7).
pub const MAX_WINDOW_S: u64 = 900;

/// A validated verdict-time window `[start_time, end_time)`, unix seconds. The upper bound is
/// exclusive so that back-to-back windows `[t, t + 900)`, `[t + 900, t + 1800)` never return
/// the same trade twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GooWindow {
    pub start_time: u64,
    pub end_time: u64,
}

/// `start_time` is required; `end_time` defaults to `start_time + 900`. Rejects an empty
/// window (`end_time <= start_time`) and windows wider than [`MAX_WINDOW_S`].
pub fn validate_window(
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> Result<GooWindow, String> {
    let start_time = start_time.ok_or_else(|| "start_time is required".to_string())?;
    let end_time = end_time.unwrap_or_else(|| start_time.saturating_add(MAX_WINDOW_S));
    if end_time <= start_time {
        return Err("end_time must be after start_time".to_string());
    }
    if end_time - start_time > MAX_WINDOW_S {
        return Err(format!(
            "window too wide: end_time - start_time must be at most {} seconds",
            MAX_WINDOW_S
        ));
    }
    Ok(GooWindow {
        start_time,
        end_time,
    })
}

/// Derive `local_origin_record` certificates from trades that reached `Executed`
/// within `window`.
///
/// Only `Executed` trades qualify: `Executed` is the delivery-verified status, so the
/// exchange attests to the traded quantity. The unit of issuance is the trade, not the
/// metered volume — these certify *traded* energy.
///
/// The window bounds **when the trade was validated** (`status_updated_at`), not when the
/// energy flowed: metering arrives after the interval it describes and by a variable delay,
/// so a window over delivery time would advance past slots still awaiting their verdict.
#[tracing::instrument(name = "Derive guarantees of origin from executed trades", skip(db))]
pub async fn guarantees_of_origin(
    db: &DatabaseWrapper,
    window: GooWindow,
) -> Result<Vec<LocalOriginRecord>> {
    let trades = db
        .trades()
        .filter_trades_by_status_change(
            window.start_time,
            Some(window.end_time),
            Some(TradeStatus::Executed),
        )
        .await?;
    if trades.is_empty() {
        return Ok(Vec::new());
    }

    // The window above is on verdict time; measurements and the allocation set are keyed by
    // delivery slot, so bound those fetches by the delivery slots of the selected trades.
    let (earliest_slot, latest_slot) = trades.iter().fold((u64::MAX, 0), |(min, max), trade| {
        (min.min(trade.time_slot), max.max(trade.time_slot))
    });

    // A facility's net export is allocated over every `Executed` sale of the slot, not just
    // those the window selected, so a trade's certificate does not depend on the window it
    // is queried with. `filter_trades` is end-exclusive; an overflowing bound widens to
    // unbounded, since a superset is always correct here.
    let slot_executed = db
        .trades()
        .filter_trades(Some(earliest_slot), latest_slot.checked_add(1))
        .await?
        .into_iter()
        .filter(|trade| trade.status == TradeStatus::Executed)
        .collect::<Vec<_>>();

    let facilities = db.facilities().get_all().await?;
    let communities = db.communities().get_all().await?;
    let assets = db.assets().get_all().await?;
    let measurement_points = db
        .measurement_points()
        .filter_points(None, Some(MeasurementPointType::Measurement))
        .await?;
    let timeseries = db
        .timeseries()
        .filter_values(
            None,
            Some(timestamp_to_string_with_padding(earliest_slot)),
            Some(timestamp_to_string_with_padding(latest_slot)),
        )
        .await?;

    let inputs = CertificateInputs {
        facilities: &facilities,
        communities: &communities,
        assets: &assets,
        measurement_points: &measurement_points,
        timeseries: &timeseries,
    };
    let mut records = build_local_origin_records_with_allocation(trades, &slot_executed, &inputs);
    sort_records(&mut records);
    Ok(records)
}
