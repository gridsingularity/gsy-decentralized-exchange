//! Communities, sites and facilities keep one document per change. Reads show the latest
//! document of each id unless its `valid_until` has passed.

use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::db::grid_topology_service::NameConflict;
use gsy_offchain_storage::db::DatabaseWrapper;
use mongodb::bson::{doc, Document};
use primitives::db_api_schema::grid_topology::{FacilitySchema, SiteSchema};
use primitives::ewds::dto::{EwdsFacilityDto, EwdsSiteDto};
use primitives::ewds::is_invalid_event;
use primitives::utils::{epoch_to_rfc3339, now_epoch_secs, EPOCH_RFC3339};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

const DAY: u64 = 86_400;

fn facility(facility_id: &str, site_id: &str) -> FacilitySchema {
    FacilitySchema {
        facility_id: facility_id.to_string(),
        facility_name: format!("Facility {facility_id}"),
        site_id: site_id.to_string(),
        owner_id: "owner-1".to_string(),
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
    }
}

fn site(site_id: &str) -> SiteSchema {
    SiteSchema {
        site_id: site_id.to_string(),
        site_name: format!("Site {site_id}"),
        site_description: String::new(),
        facilities: vec![],
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
    }
}

fn deleted<T: Clone>(entry: &T, set: impl FnOnce(&mut T)) -> T {
    let mut entry = entry.clone();
    set(&mut entry);
    entry
}

async fn documents(db: &DatabaseWrapper, collection: &str) -> u64 {
    db.collection::<Document>(collection)
        .count_documents(doc! {})
        .await
        .unwrap()
}

/// The ids of the latest facilities.
async fn latest_facility_ids(db: &DatabaseWrapper) -> Vec<String> {
    db.facilities()
        .get()
        .await
        .unwrap()
        .into_iter()
        .map(|facility| facility.facility_id)
        .collect()
}

#[tokio::test]
async fn a_change_adds_a_document_and_an_identical_resend_adds_none() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let facilities = db.facilities();

    let before = epoch_to_rfc3339(now_epoch_secs());
    let stored = facilities
        .store(vec![facility("fac-1", "site-1")])
        .await
        .unwrap();
    let after = epoch_to_rfc3339(now_epoch_secs());
    // valid_from is the receive time.
    let valid_from = &stored[0].valid_from;
    assert!(
        &before <= valid_from && valid_from <= &after,
        "{valid_from}"
    );
    // The same content again: skipped, and the stored document is returned.
    let again = facilities
        .store(vec![facility("fac-1", "site-1")])
        .await
        .unwrap();
    assert_eq!(again, stored);
    assert_eq!(documents(db, "facilities").await, 1);

    let mut changed = facility("fac-1", "site-1");
    changed.owner_id = "owner-2".to_string();
    facilities.store(vec![changed]).await.unwrap();
    assert_eq!(documents(db, "facilities").await, 2);

    let latest = facilities.get().await.unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].owner_id, "owner-2");

    stop_app(app).await;
}

#[tokio::test]
async fn names_must_be_unique_among_latest_entries_but_may_change_or_swap() {
    let app = init_app().await;
    let facilities = app.db_wrapper.facilities();
    facilities
        .store(vec![
            facility("fac-1", "site-1"),
            facility("fac-2", "site-1"),
        ])
        .await
        .unwrap();

    // fac-3 under fac-1's name.
    let mut clash = facility("fac-3", "site-1");
    clash.facility_name = "Facility fac-1".to_string();
    let error = facilities.store(vec![clash]).await.unwrap_err();
    assert!(error.downcast_ref::<NameConflict>().is_some(), "{error:#}");
    assert!(is_invalid_event(&error));

    // fac-1 and fac-2 swap their names in one batch.
    let mut first = facility("fac-1", "site-1");
    first.facility_name = "Facility fac-2".to_string();
    let mut second = facility("fac-2", "site-1");
    second.facility_name = "Facility fac-1".to_string();
    facilities.store(vec![first, second]).await.unwrap();
    let names: Vec<(String, String)> = facilities
        .get()
        .await
        .unwrap()
        .into_iter()
        .map(|facility| (facility.facility_id, facility.facility_name))
        .collect();
    assert_eq!(
        names,
        vec![
            ("fac-1".to_string(), "Facility fac-2".to_string()),
            ("fac-2".to_string(), "Facility fac-1".to_string()),
        ]
    );

    // Once fac-1 is deleted, its name is free.
    let mut gone = facility("fac-1", "site-1");
    gone.facility_name = "Facility fac-2".to_string();
    gone.valid_until = Some(epoch_to_rfc3339(now_epoch_secs()));
    facilities.store(vec![gone]).await.unwrap();
    let mut reuse = facility("fac-3", "site-1");
    reuse.facility_name = "Facility fac-2".to_string();
    facilities.store(vec![reuse]).await.unwrap();

    stop_app(app).await;
}

#[tokio::test]
async fn valid_until_hides_an_entry_from_that_time_on_and_a_resend_brings_it_back() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let facilities = db.facilities();
    facilities
        .store(vec![
            facility("fac-1", "site-1"),
            facility("fac-2", "site-1"),
            facility("fac-3", "site-1"),
        ])
        .await
        .unwrap();

    // fac-1 is deleted at once, fac-2 is announced to go tomorrow, fac-3 in two seconds.
    let now = now_epoch_secs();
    let soon = now + 2;
    facilities
        .store(vec![
            deleted(&facility("fac-1", "site-1"), |f| {
                f.valid_until = Some(epoch_to_rfc3339(now))
            }),
            deleted(&facility("fac-2", "site-1"), |f| {
                f.valid_until = Some(epoch_to_rfc3339(now + DAY))
            }),
            deleted(&facility("fac-3", "site-1"), |f| {
                f.valid_until = Some(epoch_to_rfc3339(soon))
            }),
        ])
        .await
        .unwrap();
    assert_eq!(latest_facility_ids(db).await, vec!["fac-2", "fac-3"]);
    while now_epoch_secs() < soon {
        sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(latest_facility_ids(db).await, vec!["fac-2"]);

    // Sending the deletion again adds nothing.
    let before = documents(db, "facilities").await;
    facilities
        .store(vec![deleted(&facility("fac-1", "site-1"), |f| {
            f.valid_until = Some(epoch_to_rfc3339(now))
        })])
        .await
        .unwrap();
    assert_eq!(documents(db, "facilities").await, before);

    // Sending fac-3 again without valid_until brings it back.
    facilities
        .store(vec![facility("fac-3", "site-1")])
        .await
        .unwrap();
    assert_eq!(latest_facility_ids(db).await, vec!["fac-2", "fac-3"]);

    stop_app(app).await;
}

#[tokio::test]
async fn the_rest_endpoints_store_versions_and_show_only_latest_entries() {
    let app = init_app().await;
    let client = reqwest::Client::new();
    let post = |path: &'static str, body: serde_json::Value| {
        client
            .post(format!("{}/{path}", app.address))
            .json(&body)
            .send()
    };

    let response = post("sites", json!(EwdsSiteDto::from(site("site-1"))))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let stored: EwdsSiteDto = response.json().await.unwrap();
    // valid_from is the receive time, whatever the client sent.
    assert_ne!(stored.valid_from.as_deref(), Some(EPOCH_RFC3339));

    for facility_id in ["fac-1", "fac-2"] {
        let response = post(
            "facilities",
            json!(EwdsFacilityDto::from(facility(facility_id, "site-1"))),
        )
        .await
        .unwrap();
        assert_eq!(response.status().as_u16(), 200);
    }

    // A name of another facility is a conflict, an invalid valid_until is a bad request.
    let mut clash = EwdsFacilityDto::from(facility("fac-3", "site-1"));
    clash.facility_name = "Facility fac-1".to_string();
    assert_eq!(
        post("facilities", json!(clash)).await.unwrap().status(),
        409
    );
    let mut invalid = EwdsFacilityDto::from(facility("fac-3", "site-1"));
    invalid.valid_until = Some("tomorrow".to_string());
    assert_eq!(
        post("facilities", json!(invalid)).await.unwrap().status(),
        400
    );

    // fac-2 is deleted from now on, so GET leaves it out.
    let mut gone = EwdsFacilityDto::from(facility("fac-2", "site-1"));
    gone.valid_until = Some(epoch_to_rfc3339(now_epoch_secs()));
    assert_eq!(post("facilities", json!(gone)).await.unwrap().status(), 200);
    let listed: Vec<EwdsFacilityDto> = client
        .get(format!("{}/facilities", app.address))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .map(|facility| facility.facility_id.as_str())
            .collect::<Vec<_>>(),
        vec!["fac-1"]
    );

    stop_app(app).await;
}
