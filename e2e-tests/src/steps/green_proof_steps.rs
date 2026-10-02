//! Steps of `features/green_proofs`: guarantees of origin (`local_origin_record`s) derived
//! by offchain storage from `Executed` trades, queried over EWDS
//! (`guarantees_of_origin.query`) and over the REST twin
//! (`GET /guarantees-of-origin-measurements`).
//!
//! No verdict pipeline marks trades `Executed` yet, so the scenarios seed the topology,
//! measurements and an already-`Executed` trade directly into Mongo. Every id is unique per
//! scenario run; the seeded documents are deleted by the last step (and the whole database
//! is dropped after every scenario by `main.rs` anyway).

use crate::world::{GreenProofSeed, MyWorld};
use cucumber::{given, then, when};
use gsy_community_client::time_utils::get_current_timestamp_in_secs;
use mongodb::bson::{doc, Document};
use mongodb::options::ClientOptions;
use mongodb::Database;
use primitives::certificates::{
    AssetClass, DeliveryScope, FlowDirection as CertificateFlowDirection, LocalOriginRecord,
    RecordType, TradeStatusAtIssuance,
};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema, SiteSchema,
};
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters, TradeStatus};
use primitives::ewds::{EwdsClient, EwdsOperation};
use primitives::utils::{
    bytes16_to_hex, create_encrypted_bytes16_from_string, timestamp_to_datetime_string,
    timestamp_to_string_with_padding,
};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

/// Slot length and the maximum query window of the endpoint (D7), in seconds.
const SLOT_S: u64 = 900;
/// How long a query is retried until it yields the expected kind of answer. Each EWDS
/// query additionally has its own response timeout (`EWDS_RESPONSE_TIMEOUT_MS`).
const EWDS_RETRY_BUDGET: Duration = Duration::from_secs(60);
const EWDS_RETRY_INTERVAL: Duration = Duration::from_secs(3);
const REST_PATH: &str = "guarantees-of-origin-measurements";

async fn database() -> Database {
    let db_url = std::env::var("MONGO_URL").unwrap_or_else(|_| {
        "mongodb://gsy:gsy@mongodb:27017/?retryWrites=true&w=majority".to_string()
    });
    let db_name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "offchain_storage".to_string());
    let options = ClientOptions::parse(&db_url)
        .await
        .expect("Invalid MONGO_URL");
    mongodb::Client::with_options(options)
        .expect("Failed to create the Mongo client")
        .database(db_name.as_str())
}

/// The on-chain party hash of an off-chain owner id, as stored in `DbTradeSchema::seller`.
fn party_hash(owner_id: &str) -> String {
    bytes16_to_hex(create_encrypted_bytes16_from_string(owner_id))
}

/// `2026-05-14T10:30:00+00:00`, the format of `interval_start` / `interval_end`
/// (chrono's `to_rfc3339` of a whole-second UTC instant).
fn rfc3339_utc(timestamp: u64) -> String {
    // `timestamp_to_datetime_string` is `%Y-%m-%d %H:%M:%S.%f`.
    let datetime = timestamp_to_datetime_string(timestamp);
    format!("{}+00:00", datetime[..19].replacen(' ', "T", 1))
}

fn window_around(instant: u64) -> (u64, u64) {
    (instant - SLOT_S / 2, instant + SLOT_S / 2)
}

fn seed(world: &MyWorld) -> &GreenProofSeed {
    world
        .green_proof
        .seed
        .as_ref()
        .expect("No green-proof data was seeded in this scenario")
}

fn window(world: &MyWorld) -> (u64, u64) {
    world
        .green_proof
        .window
        .expect("No guarantees-of-origin query was made in this scenario")
}

// --- Seeding -------------------------------------------------------------------------

#[given(expr = "an executed PV sale of {float} kWh is seeded with a seller export of {float} kWh")]
async fn seed_sale_with_export(world: &mut MyWorld, energy_kwh: f64, export_kwh: f64) {
    seed_executed_pv_sale(world, energy_kwh, Some(export_kwh)).await;
}

#[given(expr = "an executed PV sale of {float} kWh is seeded without a seller export measurement")]
async fn seed_sale_without_export(world: &mut MyWorld, energy_kwh: f64) {
    seed_executed_pv_sale(world, energy_kwh, None).await;
}

/// Seeds a community with one site, a seller facility with a PV asset and a buyer facility
/// on that site, optionally the seller's export measurement at the slot, and one sale from
/// the seller to the buyer that reached `Executed` just now.
async fn seed_executed_pv_sale(world: &mut MyWorld, energy_kwh: f64, export_kwh: Option<f64>) {
    let run = Uuid::new_v4().simple().to_string();
    let now = get_current_timestamp_in_secs();
    // The last fully elapsed slot: delivered, and 900-aligned as the builder requires.
    let time_slot = now - now % SLOT_S - SLOT_S;

    let community_id = Uuid::new_v4().to_string();
    let seller_facility_id = format!("goo-e2e-seller-{run}");
    let seed = GreenProofSeed {
        site_id: format!("goo-e2e-site-{run}"),
        seller_owner_id: format!("goo-e2e-seller-owner-{run}"),
        buyer_owner_id: format!("goo-e2e-buyer-owner-{run}"),
        buyer_facility_id: format!("goo-e2e-buyer-{run}"),
        pv_asset_uuid: format!("goo-e2e-pv-{run}"),
        measurement_id: export_kwh
            .map(|_| format!("Measurement:{community_id}:{seller_facility_id}")),
        trade_uuid: Uuid::new_v4().to_string(),
        time_slot,
        status_updated_at: now,
        energy_kwh,
        community_id,
        seller_facility_id,
    };

    let db = database().await;

    db.collection::<EnergyCommunitySchema>("communities")
        .insert_one(EnergyCommunitySchema {
            community_id: seed.community_id.clone(),
            community_name: format!("GoO E2E Community {run}"),
            sites: vec![seed.site_id.clone()],
        })
        .await
        .expect("Failed to seed the community");

    db.collection::<SiteSchema>("sites")
        .insert_one(SiteSchema {
            site_name: seed.site_id.clone(),
            site_description: "Guarantees-of-origin e2e site".to_string(),
            facilities: vec![
                seed.seller_facility_id.clone(),
                seed.buyer_facility_id.clone(),
            ],
        })
        .await
        .expect("Failed to seed the site");

    let facilities = [
        (&seed.seller_facility_id, &seed.seller_owner_id),
        (&seed.buyer_facility_id, &seed.buyer_owner_id),
    ]
    .into_iter()
    .map(|(facility_id, owner_id)| FacilitySchema {
        facility_id: facility_id.clone(),
        facility_name: format!("{facility_id}-name"),
        site_id: seed.site_id.clone(),
        owner_id: owner_id.clone(),
    })
    .collect::<Vec<_>>();
    db.collection::<FacilitySchema>("facilities")
        .insert_many(facilities)
        .await
        .expect("Failed to seed the facilities");

    db.collection::<AssetSchema>("assets")
        .insert_one(AssetSchema {
            asset_type: AssetType::PV,
            uuid: seed.pv_asset_uuid.clone(),
            asset_name: seed.pv_asset_uuid.clone(),
            facility_name: seed.seller_facility_id.clone(),
            creation_time: now,
            installed_power: Some(10.0),
            asset_subtype: None,
            technology_type: None,
            phase_connection: None,
            energy_capacity: None,
            maximum_soc: None,
            minimum_soc: None,
            roundtrip_efficiency: None,
            target_service: None,
            grid_connection_type: None,
            max_rated_current: None,
            has_smart_meter: None,
            tariff_name: None,
        })
        .await
        .expect("Failed to seed the PV asset");

    if let (Some(measurement_id), Some(export_kwh)) = (seed.measurement_id.as_ref(), export_kwh) {
        db.collection::<MeasurementPointSchema>("measurement_points")
            .insert_one(MeasurementPointSchema {
                point_type: MeasurementPointType::Measurement,
                measurement_id: measurement_id.clone(),
                property_measured: "energy_measured".to_string(),
                unit: "kWh".to_string(),
                direction: FlowDirection::Export,
                energy_accumulated: false,
                time_resolution: "PT15M".to_string(),
                phase: 0,
                asset_name: seed.seller_facility_id.clone(),
                datasource_name: Some(seed.community_id.clone()),
            })
            .await
            .expect("Failed to seed the measurement point");
        db.collection::<TimeseriesSchema>("timeseries")
            .insert_one(TimeseriesSchema {
                measurement_point: measurement_id.clone(),
                timestamp: timestamp_to_string_with_padding(time_slot),
                value: export_kwh,
            })
            .await
            .expect("Failed to seed the export measurement");
    }

    db.collection::<DbTradeSchema>("trades")
        .insert_one(DbTradeSchema {
            trade_uuid: seed.trade_uuid.clone(),
            status: TradeStatus::Executed,
            seller: party_hash(&seed.seller_owner_id),
            buyer: party_hash(&seed.buyer_owner_id),
            market_id: format!("0xgooe2e{run}"),
            time_slot,
            creation_time: time_slot - SLOT_S,
            offer_hash: format!("{}-offer", seed.trade_uuid),
            bid_hash: format!("{}-bid", seed.trade_uuid),
            residual_offer_id: None,
            residual_bid_id: None,
            parameters: TradeParameters {
                selected_energy_kWh: energy_kwh,
                energy_rate: 0.2,
            },
            status_updated_at: Some(now),
        })
        .await
        .expect("Failed to seed the executed trade");

    info!(
        "Seeded executed PV sale {} (slot={}, status_updated_at={}, seller={}, export={:?})",
        seed.trade_uuid, time_slot, now, seed.seller_facility_id, export_kwh
    );
    world.green_proof.seed = Some(seed);
}

// --- Queries ---------------------------------------------------------------------------

async fn ewds_query(window: (u64, u64)) -> Result<Vec<LocalOriginRecord>, String> {
    EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000)
        .query::<LocalOriginRecord>(
            EwdsOperation::GuaranteesOfOriginQuery,
            json!({"startTime": window.0, "endTime": window.1}),
        )
        .await
        .map_err(|error| format!("{error:#}"))
}

/// Repeats the EWDS query until `done` accepts the outcome or [`EWDS_RETRY_BUDGET`]
/// elapses, then records the last outcome in the world.
async fn query_ewds_until<F>(world: &mut MyWorld, window: (u64, u64), done: F)
where
    F: Fn(&Result<Vec<LocalOriginRecord>, String>) -> bool,
{
    let started = Instant::now();
    let mut attempt = 0u32;
    let outcome = loop {
        attempt += 1;
        let outcome = ewds_query(window).await;
        if done(&outcome) || started.elapsed() >= EWDS_RETRY_BUDGET {
            break outcome;
        }
        info!(
            "guarantees_of_origin.query attempt {} not conclusive yet ({}); retrying",
            attempt,
            match &outcome {
                Ok(records) => format!("{} record(s)", records.len()),
                Err(error) => error.clone(),
            }
        );
        sleep(EWDS_RETRY_INTERVAL).await;
    };

    world.green_proof.window = Some(window);
    match outcome {
        Ok(records) => {
            world.green_proof.ewds_records = records;
            world.green_proof.ewds_error = None;
        }
        Err(error) => {
            world.green_proof.ewds_records = vec![];
            world.green_proof.ewds_error = Some(error);
        }
    }
}

#[when(
    "the guarantees of origin around the verdict time are queried over EWDS until the seeded sale is certified"
)]
async fn query_until_certified(world: &mut MyWorld) {
    let trade_uuid = seed(world).trade_uuid.clone();
    let window = window_around(seed(world).status_updated_at);
    query_ewds_until(world, window, |outcome| {
        matches!(outcome, Ok(records) if records
            .iter()
            .any(|record| record.trade_and_delivery.trade_reference.contains(&trade_uuid)))
    })
    .await;
}

#[when("the guarantees of origin around the verdict time are queried over EWDS")]
async fn query_once(world: &mut MyWorld) {
    let window = window_around(seed(world).status_updated_at);
    query_ewds_until(world, window, |outcome| outcome.is_ok()).await;
}

async fn rest_query(world: &MyWorld, window: (u64, u64)) -> reqwest::Response {
    world
        .http_client
        .get(format!("{}/{}", world.offchain_storage_url, REST_PATH))
        .query(&[("start_time", window.0), ("end_time", window.1)])
        .send()
        .await
        .expect("Failed to contact the guarantees-of-origin REST endpoint")
}

// --- Assertions ------------------------------------------------------------------------

fn ewds_records(world: &MyWorld) -> &[LocalOriginRecord] {
    if let Some(error) = world.green_proof.ewds_error.as_ref() {
        panic!("guarantees_of_origin.query failed: {error}");
    }
    &world.green_proof.ewds_records
}

#[then("exactly one local origin record is returned for the seeded sale")]
async fn exactly_one_record(world: &mut MyWorld) {
    let trade_uuid = seed(world).trade_uuid.clone();
    let records = ewds_records(world);
    assert_eq!(
        records.len(),
        1,
        "Expected exactly one local_origin_record over EWDS, got {}: {:#?}",
        records.len(),
        records
    );
    assert_eq!(
        records[0].trade_and_delivery.trade_reference,
        vec![trade_uuid],
        "The record does not reference the seeded trade"
    );
}

#[then(
    expr = "the record certifies {float} kWh of the seller facility for the seeded slot within its community"
)]
async fn record_fields(world: &mut MyWorld, energy_quantity: f64) {
    let seed = seed(world).clone();
    let record = ewds_records(world)
        .first()
        .expect("No local_origin_record was returned")
        .clone();

    assert_eq!(record.identity.record_type, RecordType::LocalOriginRecord);
    assert_eq!(record.identity.site_id, seed.site_id);

    assert_eq!(
        record.production_asset.production_asset_id,
        seed.seller_facility_id
    );
    assert_eq!(record.production_asset.asset_class, AssetClass::Pv);
    assert_eq!(
        record.consumption_asset.consumption_asset_id,
        seed.buyer_facility_id
    );

    assert!(
        (record.time_and_quantity.energy_quantity - energy_quantity).abs() < 1e-9,
        "energy_quantity: expected {} (traded {} kWh rounded to 2 dp), got {}",
        energy_quantity,
        seed.energy_kwh,
        record.time_and_quantity.energy_quantity
    );
    assert_eq!(
        record.time_and_quantity.source_slot_timestamp,
        seed.time_slot
    );
    assert_eq!(
        record.time_and_quantity.interval_start,
        rfc3339_utc(seed.time_slot)
    );
    assert_eq!(
        record.time_and_quantity.interval_end,
        rfc3339_utc(seed.time_slot + SLOT_S)
    );

    assert_eq!(
        record.location.delivery_scope,
        DeliveryScope::IntraCommunity
    );
    assert_eq!(
        record.location.community_id_origin.as_deref(),
        Some(seed.community_id.as_str())
    );
    assert_eq!(
        record.location.community_id_consumption.as_deref(),
        Some(seed.community_id.as_str())
    );

    assert_eq!(
        Some(&record.measurement_provenance.measurement_id),
        seed.measurement_id.as_ref()
    );
    assert_eq!(
        record.measurement_provenance.flow_direction,
        CertificateFlowDirection::Export
    );

    assert_eq!(
        record.trade_and_delivery.trade_reference,
        vec![seed.trade_uuid.clone()]
    );
    assert_eq!(
        record.trade_and_delivery.trade_status_at_issuance,
        Some(TradeStatusAtIssuance::DeliveryVerified)
    );
}

#[then("no local origin record is returned")]
async fn no_record(world: &mut MyWorld) {
    let records = ewds_records(world);
    assert!(
        records.is_empty(),
        "Expected no local_origin_record over EWDS, got {}: {:#?}",
        records.len(),
        records
    );
}

#[then("the REST endpoint returns the same records")]
async fn rest_returns_same_records(world: &mut MyWorld) {
    let response = rest_query(world, window(world)).await;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    assert!(
        status.is_success(),
        "GET /{} failed with status {}: {}",
        REST_PATH,
        status,
        body
    );
    let rest_records: Vec<LocalOriginRecord> = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("Invalid GET /{} response ({e}): {body}", REST_PATH));
    assert_eq!(
        rest_records,
        ewds_records(world),
        "The REST twin and EWDS returned different records"
    );
}

// --- Cleanup ---------------------------------------------------------------------------

#[then("the seeded green-proof documents are deleted")]
async fn delete_seeded_documents(world: &mut MyWorld) {
    let Some(seed) = world.green_proof.seed.take() else {
        return;
    };
    let db = database().await;
    let mut deletions: Vec<(&str, Document)> = vec![
        ("trades", doc! {"trade_uuid": &seed.trade_uuid}),
        (
            "facilities",
            doc! {"facility_id": {"$in": [&seed.seller_facility_id, &seed.buyer_facility_id]}},
        ),
        ("assets", doc! {"uuid": &seed.pv_asset_uuid}),
        ("sites", doc! {"site_name": &seed.site_id}),
        ("communities", doc! {"community_id": &seed.community_id}),
    ];
    if let Some(measurement_id) = seed.measurement_id.as_ref() {
        deletions.push((
            "measurement_points",
            doc! {"measurement_id": measurement_id},
        ));
        deletions.push(("timeseries", doc! {"measurement_point": measurement_id}));
    }
    for (collection, filter) in deletions {
        db.collection::<Document>(collection)
            .delete_many(filter)
            .await
            .unwrap_or_else(|e| panic!("Failed to delete seeded {collection}: {e}"));
    }
}
