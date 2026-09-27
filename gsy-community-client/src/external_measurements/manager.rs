//! Measurement ingestion. FLEXO meters report to InfluxDB as `FLEXO-<site>-<T>-<type>`; the
//! token `T` belongs to the ontology SmartMeter `T + "SM"`, and a building is measured by the
//! sum of its SmartMeters' net grid exchange (see `metering_points`). Every tick re-reads a
//! look-back window, because FLEXO data lands in batches, sometimes more than a day late: a
//! slot is posted as soon as every meter of a point has reported, and as incomplete or
//! missing only once it is older than the grace period. Storage upserts on
//! `(area_hash, time_slot)`, so re-posting a slot is harmless.

use crate::constants::CommunityClientConstants;
use crate::external_measurements::influxdb_api::MeasurementInfluxDBConnection;
use crate::external_measurements::metering_points::{
    MeteringPointKind, MeteringPointSet, ended_slots, measurement_rows, unmapped_tokens,
};
use crate::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use chrono::{DateTime, Utc};
use gsy_offchain_primitives::constants::GlobalConstants;
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema,
};
use gsy_offchain_primitives::utils::read_env_or;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone)]
pub struct MeasurementsManager {
    external_measurements_api: MeasurementInfluxDBConnection,
    offchain_storage_api: AreaMarketInfoAdapter,
}

/// Number of measurement rows of a community per completeness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletenessCounts {
    pub complete: usize,
    pub incomplete: usize,
    pub missing: usize,
}

/// An `Incomplete` row: some, but not all, expected meters of the point reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteRow {
    pub community: String,
    pub point: String,
    pub time_slot: u64,
    pub missing_meters: Vec<String>,
}

/// What a set of measurement rows holds, per community.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeasurementRowSummary {
    /// Every community of the metering point set, with zero counts if it has no rows.
    pub counts: BTreeMap<String, CompletenessCounts>,
    /// Sorted by `(community, point, time_slot)`.
    pub incomplete: Vec<IncompleteRow>,
}

/// The outcome of one [`MeasurementsManager::ingest`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeasurementIngestReport {
    /// The rows that were (to be) forwarded.
    pub summary: MeasurementRowSummary,
    /// Number of meter tokens with readings in the window.
    pub meters_with_readings: usize,
    /// Tokens with readings but no SmartMeter in the ontology.
    pub unmapped_tokens: BTreeSet<String>,
    /// InfluxDB returned nothing for the window, which is treated as a failed read: nothing
    /// is forwarded, so an outage never turns into rows posted as missing.
    pub no_readings: bool,
    /// Number of rows sent to storage; if `forward_error` is set, the number attempted.
    pub forwarded: usize,
    /// The error of the forward, if it failed.
    pub forward_error: Option<String>,
}

/// Count `rows` per community and completeness, and list the incomplete ones. A row belongs
/// to the point of `set` with its `area_hash`; rows of no point are not counted.
pub fn summarize_rows(set: &MeteringPointSet, rows: &[MeasurementSchema]) -> MeasurementRowSummary {
    let mut summary = MeasurementRowSummary::default();
    let mut point_of_hash: HashMap<&str, (&str, &str)> = HashMap::new();
    for point in &set.points {
        summary
            .counts
            .entry(point.community_name.clone())
            .or_default();
        point_of_hash.insert(
            point.area_hash.as_str(),
            (point.community_name.as_str(), point.name.as_str()),
        );
    }

    for row in rows {
        let (Some(metering_point), Some(&(community, point))) = (
            row.metering_point.as_ref(),
            point_of_hash.get(row.area_hash.as_str()),
        ) else {
            continue;
        };
        let counts = summary.counts.entry(community.to_string()).or_default();
        match metering_point.completeness {
            MeasurementCompleteness::Complete => counts.complete += 1,
            MeasurementCompleteness::Missing => counts.missing += 1,
            MeasurementCompleteness::Incomplete => {
                counts.incomplete += 1;
                summary.incomplete.push(IncompleteRow {
                    community: community.to_string(),
                    point: point.to_string(),
                    time_slot: row.time_slot,
                    missing_meters: metering_point.missing_meters.clone(),
                });
            }
        }
    }

    summary.incomplete.sort_by(|a, b| {
        (&a.community, &a.point, a.time_slot).cmp(&(&b.community, &b.point, b.time_slot))
    });
    summary
}

/// One line per community of `set`: its building points, and each unmetered point with its
/// members.
pub fn describe_metering_points(set: &MeteringPointSet) -> Vec<String> {
    let mut communities: BTreeMap<&str, (usize, Vec<String>)> = BTreeMap::new();
    for point in &set.points {
        let (buildings, unmetered) = communities
            .entry(point.community_name.as_str())
            .or_default();
        match point.kind {
            MeteringPointKind::Building => *buildings += 1,
            MeteringPointKind::UnmeteredSite => {
                let members: Vec<&str> = point.members.iter().map(String::as_str).collect();
                unmetered.push(format!("{} ({})", point.name, members.join(", ")));
            }
        }
    }
    communities
        .into_iter()
        .map(|(community, (buildings, unmetered))| {
            let unmetered_text = if unmetered.is_empty() {
                "no unmetered point".to_string()
            } else {
                format!(
                    "{} unmetered point(s), always missing: {}",
                    unmetered.len(),
                    unmetered.join("; ")
                )
            };
            format!("{community}: {buildings} building point(s), {unmetered_text}")
        })
        .collect()
}

impl MeasurementsManager {
    pub fn new() -> Self {
        MeasurementsManager {
            external_measurements_api: MeasurementInfluxDBConnection::new(),
            // Must read OFFCHAIN_STORAGE_URL like the forecast path does. Defaulting to
            // the `gsy-orderbook` alias only resolves under docker-compose.local-demo.yml,
            // which defines that alias; on the two-host stack every service shares the
            // Tailscale sidecar's namespace and has no DNS name at all, so a hardcoded
            // host there would send every measurement into a dead name.
            offchain_storage_api: AreaMarketInfoAdapter::new(Some(read_env_or(
                "OFFCHAIN_STORAGE_URL",
                "http://gsy-orderbook:8080".to_string(),
            ))),
        }
    }

    /// Read InfluxDB over `[now - MEASUREMENT_LOOKBACK_SEC, now]`, build the row of every
    /// point of `set` for every ended slot in that window, and forward the valid ones.
    /// Never panics: every failure is reported instead.
    pub async fn ingest(&self, set: &MeteringPointSet, now: u64) -> MeasurementIngestReport {
        let lookback_sec = CommunityClientConstants.MEASUREMENT_LOOKBACK_SEC;
        let missing_after_sec = CommunityClientConstants.MEASUREMENT_MISSING_AFTER_SEC;
        let to_time = |secs: u64| {
            i64::try_from(secs)
                .ok()
                .and_then(|secs| DateTime::<Utc>::from_timestamp(secs, 0))
                .unwrap_or_default()
        };

        let readings = self
            .external_measurements_api
            .read(to_time(now.saturating_sub(lookback_sec)), to_time(now))
            .await;
        let slots = ended_slots(now, lookback_sec, GlobalConstants.TIME_SLOT_SEC);
        let rows: Vec<MeasurementSchema> =
            measurement_rows(&set.points, &readings, &slots, now, missing_after_sec)
                .into_iter()
                .filter(|row| self.offchain_storage_api.validate_measurement(row, now))
                .collect();

        let mut report = MeasurementIngestReport {
            summary: summarize_rows(set, &rows),
            meters_with_readings: readings.values().filter(|slots| !slots.is_empty()).count(),
            unmapped_tokens: unmapped_tokens(set, &readings),
            ..MeasurementIngestReport::default()
        };
        if report.meters_with_readings == 0 {
            report.no_readings = true;
            return report;
        }

        report.forwarded = rows.len();
        if let Err(error) = self.offchain_storage_api.forward_measurement(rows).await {
            report.forward_error = Some(error.to_string());
        }
        report
    }
}
