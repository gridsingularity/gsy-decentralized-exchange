//! Metering points: one per building, measured by the sum of the building's Influx meters,
//! plus one unmetered point per site for the assets located at the site itself rather than
//! in one of its buildings. Everything here is pure (no I/O): the points are built from the
//! raw ontology responses, and the measurement rows from the Influx readings.
//!
//! An Influx meter token `T` (the `<meter>` of `FLEXO-<site>-<meter>-<type>`, e.g. `AIC01`)
//! belongs to the ontology SmartMeter asset `T + "SM"`. The ids are the ones markets derive,
//! so a trade side's area hash is found among a point's member hashes.

use crate::constants::EXCLUDED_METERS;
use crate::external_measurements::influxdb_api::InfluxMeasurementMeterData;
use crate::offchain_storage_connector::adapter::{
    deterministic_area_hash, deterministic_area_uuid, deterministic_community_uuid,
};
use crate::topology::{
    ExternalCommunityAsset, LECCommunityAssetsResults, LECCommunityMembersResults,
};
use chrono::{DateTime, Utc};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
};
use gsy_offchain_primitives::utils::h256_to_string;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

/// `assetType` of every meter, compared without the URI scheme.
const METER_TYPE: &str = "w3id.org/fedecom/energyasset#Meter";
/// `assetSubType` of a SmartMeter, compared without the URI scheme.
const SMART_METER_SUB_TYPE: &str = "w3id.org/fedecom/energyasset#SmartMeter";
/// Suffix of the SmartMeter asset `T + "SM"` that the Influx meter token `T` belongs to.
const SMART_METER_SUFFIX: &str = "SM";

/// Influx readings as returned by `MeasurementInfluxDBConnection::read`: per meter token,
/// per interval start.
pub type MeterReadings = HashMap<String, HashMap<DateTime<Utc>, InfluxMeasurementMeterData>>;

/// Whether a metering point is a building or the site-level assets of a site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeteringPointKind {
    /// A building (`participantName`), measured by the sum of its expected meters.
    Building,
    /// The assets located at a site (`siteName`) itself. It expects no meter, so each of
    /// its rows is `Missing`.
    UnmeteredSite,
}

/// One metering point: a building, or the site-level assets of a site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteringPoint {
    pub community_name: String,
    /// `deterministic_community_uuid(community_name)`.
    pub community_uuid: String,
    /// Building `participantName`, or the `siteName` of an unmetered site point.
    pub name: String,
    pub kind: MeteringPointKind,
    /// `deterministic_area_uuid(community_name, name)`.
    pub area_uuid: String,
    /// `deterministic_area_hash(community_name, name)`.
    pub area_hash: String,
    /// Names of the member assets.
    pub members: BTreeSet<String>,
    /// `deterministic_area_hash(community_name, asset)` of every member, sorted. This is the
    /// hash a market gives the asset, so it matches the asset's trade sides.
    pub member_area_hashes: Vec<String>,
    /// Influx meter tokens whose summed net exchange is the point's measurement: every `T`
    /// with a member SmartMeter named `T + "SM"` that is not in `EXCLUDED_METERS`. Empty for
    /// an unmetered site point.
    pub expected_meters: BTreeSet<String>,
}

/// Something about the ontology worth logging when the metering points are built.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MeteringPointNote {
    /// A SmartMeter `token + "SM"` located at a site, not in a building: no point uses the
    /// readings of `token`.
    SiteLevelMeter {
        community: String,
        asset: String,
        token: String,
        site: String,
    },
    /// A SmartMeter in `EXCLUDED_METERS`: no point uses its readings.
    ExcludedMeter { community: String, asset: String },
    /// An asset whose `location` is neither a building nor a site of its community: it is
    /// in no point.
    UnknownLocation {
        community: String,
        asset: String,
        location: String,
    },
    /// An override whose target is not a building of the asset's community: ignored.
    InvalidOverride { asset: String, target: String },
    /// An override for an asset that none of the communities has: ignored.
    UnknownOverrideAsset { asset: String, target: String },
    /// A building without any expected meter: its rows are always `Missing`.
    NoExpectedMeters { community: String, building: String },
    /// A building member, other than a meter, whose name prefix is none of the building's
    /// meter tokens: it is judged against meters that are not its own.
    MemberWithoutOwnMeter {
        community: String,
        building: String,
        asset: String,
    },
}

impl fmt::Display for MeteringPointNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SiteLevelMeter {
                community,
                asset,
                token,
                site,
            } => write!(
                f,
                "{community}: meter {token} ({asset}) is located at site {site}, not in a \
                 building; no metering point uses its readings"
            ),
            Self::ExcludedMeter { community, asset } => write!(
                f,
                "{community}: {asset} is in EXCLUDED_METERS; no metering point uses its readings"
            ),
            Self::UnknownLocation {
                community,
                asset,
                location,
            } => write!(
                f,
                "{community}: {asset} has location {location}, which is neither a building nor \
                 a site of the community; it is in no metering point"
            ),
            Self::InvalidOverride { asset, target } => write!(
                f,
                "Ignoring metering point override {asset}={target}: {target} is not a building \
                 of the community of {asset}"
            ),
            Self::UnknownOverrideAsset { asset, target } => write!(
                f,
                "Ignoring metering point override {asset}={target}: no community has an asset \
                 {asset}"
            ),
            Self::NoExpectedMeters {
                community,
                building,
            } => write!(
                f,
                "{community}: building {building} has no expected meter; its measurements are \
                 always missing"
            ),
            Self::MemberWithoutOwnMeter {
                community,
                building,
                asset,
            } => write!(
                f,
                "{community}: {asset} has no meter of its own in building {building}; it is \
                 judged against the building's meters"
            ),
        }
    }
}

/// The metering points of a set of communities, and what to log about them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeteringPointSet {
    /// Sorted by `(community_name, name)`.
    pub points: Vec<MeteringPoint>,
    /// Sorted, without duplicates.
    pub notes: Vec<MeteringPointNote>,
    /// Every token `T` such that a SmartMeter named `T + "SM"` exists in one of the
    /// communities, whether or not a point expects it. Readings of any other token are
    /// unmapped (see [`unmapped_tokens`]).
    pub known_meter_tokens: BTreeSet<String>,
}

/// Build the metering points of every community in `assets` (the `get_assets` response per
/// LEC name), whose buildings and sites are read from `buildings` (the `get_lecs_buildings`
/// response). `overrides` maps an asset name to the building it belongs to, in place of its
/// ontology `location`.
///
/// A community gets points only if at least one of its buildings expects a meter. It then
/// gets one point per building, and one unmetered point per site with assets of its own.
/// The result does not depend on the order of the inputs.
pub fn build_metering_points(
    buildings: &LECCommunityMembersResults,
    assets: &[(String, LECCommunityAssetsResults)],
    overrides: &HashMap<String, String>,
) -> MeteringPointSet {
    let mut buildings_per_community: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    let mut sites_per_community: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for row in &buildings.results.bindings {
        let community = row.lec_name.value.as_str();
        buildings_per_community
            .entry(community)
            .or_default()
            .insert(row.participant_name.value.as_str());
        sites_per_community
            .entry(community)
            .or_default()
            .insert(row.site_name.value.as_str());
    }

    let mut assets_per_community: BTreeMap<&str, Vec<&ExternalCommunityAsset>> = BTreeMap::new();
    for (community, community_assets) in assets {
        assets_per_community
            .entry(community.as_str())
            .or_default()
            .extend(community_assets.results.bindings.iter());
    }

    let no_names = BTreeSet::new();
    let mut points = Vec::new();
    let mut notes = BTreeSet::new();
    let mut known_meter_tokens = BTreeSet::new();
    let mut overridden_assets = BTreeSet::new();

    for (&community, community_assets) in &assets_per_community {
        let community_buildings = buildings_per_community.get(community).unwrap_or(&no_names);
        let community_sites = sites_per_community.get(community).unwrap_or(&no_names);

        let mut building_members: BTreeMap<&str, Vec<&ExternalCommunityAsset>> = BTreeMap::new();
        let mut building_meters: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut site_members: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();

        for &asset in community_assets {
            let asset_name = asset.asset_name.value.as_str();
            let mut token = meter_token(asset);
            if let Some(token) = token {
                known_meter_tokens.insert(token.to_string());
            }
            if token.is_some() && EXCLUDED_METERS.contains(&asset_name) {
                notes.insert(MeteringPointNote::ExcludedMeter {
                    community: community.to_string(),
                    asset: asset_name.to_string(),
                });
                token = None;
            }

            let mut location = location_fragment(&asset.location.value);
            if let Some(target) = overrides.get(asset_name) {
                overridden_assets.insert(asset_name);
                if community_buildings.contains(target.as_str()) {
                    location = target.as_str();
                } else {
                    notes.insert(MeteringPointNote::InvalidOverride {
                        asset: asset_name.to_string(),
                        target: target.clone(),
                    });
                }
            }

            if community_buildings.contains(location) {
                building_members.entry(location).or_default().push(asset);
                if let Some(token) = token {
                    building_meters
                        .entry(location)
                        .or_default()
                        .insert(token.to_string());
                }
            } else if community_sites.contains(location) {
                site_members
                    .entry(location)
                    .or_default()
                    .insert(asset_name.to_string());
                if let Some(token) = token {
                    notes.insert(MeteringPointNote::SiteLevelMeter {
                        community: community.to_string(),
                        asset: asset_name.to_string(),
                        token: token.to_string(),
                        site: location.to_string(),
                    });
                }
            } else {
                notes.insert(MeteringPointNote::UnknownLocation {
                    community: community.to_string(),
                    asset: asset_name.to_string(),
                    location: asset.location.value.clone(),
                });
            }
        }

        if building_meters.is_empty() {
            continue;
        }

        for &building in community_buildings {
            let members = building_members
                .get(building)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let meters = building_meters.remove(building).unwrap_or_default();
            if meters.is_empty() {
                notes.insert(MeteringPointNote::NoExpectedMeters {
                    community: community.to_string(),
                    building: building.to_string(),
                });
            }
            for asset in members.iter().filter(|asset| !is_meter(asset)) {
                let asset_name = asset.asset_name.value.as_str();
                if name_prefix(asset_name).is_some_and(|prefix| !meters.contains(prefix)) {
                    notes.insert(MeteringPointNote::MemberWithoutOwnMeter {
                        community: community.to_string(),
                        building: building.to_string(),
                        asset: asset_name.to_string(),
                    });
                }
            }
            let member_names = members
                .iter()
                .map(|asset| asset.asset_name.value.clone())
                .collect();
            points.push(metering_point(
                community,
                building,
                MeteringPointKind::Building,
                member_names,
                meters,
            ));
        }

        for (site, members) in site_members {
            points.push(metering_point(
                community,
                site,
                MeteringPointKind::UnmeteredSite,
                members,
                BTreeSet::new(),
            ));
        }
    }

    for (asset, target) in overrides {
        if !overridden_assets.contains(asset.as_str()) {
            notes.insert(MeteringPointNote::UnknownOverrideAsset {
                asset: asset.clone(),
                target: target.clone(),
            });
        }
    }

    points.sort_by(|a, b| (&a.community_name, &a.name).cmp(&(&b.community_name, &b.name)));
    MeteringPointSet {
        points,
        notes: notes.into_iter().collect(),
        known_meter_tokens,
    }
}

/// Parse an override table `ASSET=BUILDING,ASSET=BUILDING`. Whitespace around entries and
/// names is ignored, and so are empty entries, so a blank spec gives no overrides. An entry
/// that is not exactly one non-empty `ASSET=BUILDING` pair, or that gives an asset a second,
/// different building, is an error naming it.
pub fn parse_overrides(spec: &str) -> Result<HashMap<String, String>, String> {
    let mut overrides = HashMap::new();
    let entries = spec
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty());
    for entry in entries {
        let names: Vec<&str> = entry.split('=').map(str::trim).collect();
        let [asset, building] = names[..] else {
            return Err(malformed_override(entry));
        };
        if asset.is_empty() || building.is_empty() {
            return Err(malformed_override(entry));
        }
        if let Some(previous) = overrides.insert(asset.to_string(), building.to_string()) {
            if previous != building {
                return Err(format!(
                    "Conflicting metering point override '{entry}': {asset} is already \
                     assigned to {previous}"
                ));
            }
        }
    }
    Ok(overrides)
}

/// Start of every slot of `slot_sec` seconds (a multiple of `slot_sec`) that starts no
/// earlier than `now - lookback_sec` and has ended by `now`, ascending. Every such slot is
/// listed whether or not any data exists for it; `slot_sec == 0` gives none.
pub fn ended_slots(now: u64, lookback_sec: u64, slot_sec: u64) -> Vec<u64> {
    let mut slots = Vec::new();
    if slot_sec == 0 {
        return slots;
    }
    let window_start = now.saturating_sub(lookback_sec);
    let Some(mut slot) = window_start.div_ceil(slot_sec).checked_mul(slot_sec) else {
        return slots;
    };
    while let Some(slot_end) = slot.checked_add(slot_sec) {
        if slot_end > now {
            break;
        }
        slots.push(slot);
        slot = slot_end;
    }
    slots
}

/// The measurement row of every point for every slot in `slots`, as seen at `now`. `readings`
/// is keyed by meter token, then by the interval start, which is the slot start.
///
/// A slot is `Complete` once every expected meter has both `import` and `export`, and its
/// `energy_kwh` is then their summed net exchange in kWh (positive = net import). Otherwise
/// no row is emitted until the slot is `missing_after_sec` old, since the data may still
/// land. After that the row is `Incomplete` if some expected meters have both values, or
/// `Missing` if none has (or the point expects none), with `energy_kwh = 0.0` and the meters
/// lacking a value in `missing_meters`. Readings of meters that no point expects are ignored.
pub fn measurement_rows(
    points: &[MeteringPoint],
    readings: &MeterReadings,
    slots: &[u64],
    now: u64,
    missing_after_sec: u64,
) -> Vec<MeasurementSchema> {
    let mut rows = Vec::new();
    for point in points {
        for &slot in slots {
            let slot_start = i64::try_from(slot)
                .ok()
                .and_then(|secs| DateTime::<Utc>::from_timestamp(secs, 0));
            let mut energy_kwh = 0.0;
            let mut missing_meters = Vec::new();
            for token in &point.expected_meters {
                let net_kwh =
                    slot_start.and_then(|start| readings.get(token)?.get(&start)?.net_energy_kWh());
                match net_kwh {
                    Some(net_kwh) => energy_kwh += net_kwh,
                    None => missing_meters.push(token.clone()),
                }
            }

            let completeness = if missing_meters.len() == point.expected_meters.len() {
                MeasurementCompleteness::Missing
            } else if missing_meters.is_empty() {
                MeasurementCompleteness::Complete
            } else {
                MeasurementCompleteness::Incomplete
            };
            if completeness != MeasurementCompleteness::Complete {
                let past_grace = now
                    .checked_sub(slot)
                    .is_some_and(|age| age >= missing_after_sec);
                if !past_grace {
                    continue;
                }
                energy_kwh = 0.0;
            }

            rows.push(MeasurementSchema {
                area_uuid: point.area_uuid.clone(),
                area_hash: point.area_hash.clone(),
                community_uuid: point.community_uuid.clone(),
                time_slot: slot,
                creation_time: now,
                energy_kwh,
                metering_point: Some(MeteringPointMeasurement {
                    name: point.name.clone(),
                    member_area_hashes: point.member_area_hashes.clone(),
                    completeness,
                    missing_meters,
                }),
            });
        }
    }
    rows
}

/// Tokens that have readings but no SmartMeter `T + "SM"` in any community of `set`.
pub fn unmapped_tokens(set: &MeteringPointSet, readings: &MeterReadings) -> BTreeSet<String> {
    readings
        .iter()
        .filter(|(token, meter_readings)| {
            !meter_readings.is_empty() && !set.known_meter_tokens.contains(token.as_str())
        })
        .map(|(token, _)| token.clone())
        .collect()
}

fn malformed_override(entry: &str) -> String {
    format!("Malformed metering point override '{entry}', expected ASSET=BUILDING")
}

fn metering_point(
    community: &str,
    name: &str,
    kind: MeteringPointKind,
    members: BTreeSet<String>,
    expected_meters: BTreeSet<String>,
) -> MeteringPoint {
    let mut member_area_hashes: Vec<String> = members
        .iter()
        .map(|asset| h256_to_string(deterministic_area_hash(community, asset)))
        .collect();
    member_area_hashes.sort();
    MeteringPoint {
        community_name: community.to_string(),
        community_uuid: deterministic_community_uuid(community),
        name: name.to_string(),
        kind,
        area_uuid: deterministic_area_uuid(community, name),
        area_hash: h256_to_string(deterministic_area_hash(community, name)),
        members,
        member_area_hashes,
        expected_meters,
    }
}

/// A URI without its scheme, compared the way `map_fedecom_asset_type_to_asset_type` does:
/// the ontology serves `https://` URIs, older data used `http://`.
fn strip_scheme(uri: &str) -> &str {
    uri.split_once("://").map_or(uri, |(_, rest)| rest)
}

/// The `#fragment` of a `location` URI (a building or site name), or the whole value if it
/// has none.
fn location_fragment(location: &str) -> &str {
    location
        .rsplit_once('#')
        .map_or(location, |(_, fragment)| fragment)
}

fn is_meter(asset: &ExternalCommunityAsset) -> bool {
    strip_scheme(&asset.asset_type.value) == METER_TYPE
}

/// The Influx meter token `T` of a SmartMeter asset named `T + "SM"`.
fn meter_token(asset: &ExternalCommunityAsset) -> Option<&str> {
    let is_smart_meter = is_meter(asset)
        && asset
            .asset_sub_type
            .as_ref()
            .is_some_and(|sub_type| strip_scheme(&sub_type.value) == SMART_METER_SUB_TYPE);
    if !is_smart_meter {
        return None;
    }
    asset
        .asset_name
        .value
        .strip_suffix(SMART_METER_SUFFIX)
        .filter(|token| !token.is_empty())
}

/// The leading letters of an asset name and the digits after them (`AIC44` of
/// `AIC44PV_1`), or `None` if the name does not start with letters followed by a digit.
fn name_prefix(name: &str) -> Option<&str> {
    let letters_end = name
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(name.len());
    let digits_end = name[letters_end..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(name.len(), |digits| letters_end + digits);
    (letters_end > 0 && digits_end > letters_end).then(|| &name[..digits_end])
}
