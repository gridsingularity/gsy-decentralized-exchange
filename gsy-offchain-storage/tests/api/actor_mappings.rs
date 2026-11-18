use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::ewds_event_handler::handle_event;
use mongodb::bson::doc;
use primitives::{
    db_api_schema::{grid_topology::FacilitySchema, ids::IdMappingSchema},
    ewds::{dto::EwdsEventEnvelope, EwdsEventType},
    utils::derive_onchain_id,
};
use serde_json::json;

#[tokio::test]
async fn topology_registers_actor_mappings_over_http_and_ewds_before_orders_exist() {
    let community = "11111111-1111-4111-8111-111111111111";
    let owner = "ABCDEFAB-1234-4123-8123-ABCDEFABCDEF";
    for ewds in [false, true] {
        let app = init_app().await;
        let http = reqwest::Client::new();
        for (route, event_type, data) in [
            (
                "facilities",
                EwdsEventType::Facility,
                json!({
                    "facility_id": "LIC01", "facility_name": "PV", "owner_id": owner, "site_id": "Site-A"
                }),
            ),
            (
                "sites",
                EwdsEventType::Site,
                json!({
                    "site_name": "Site-A", "site_description": "Site", "facilities": ["LIC01", "not-yet-registered"]
                }),
            ),
            (
                "communities",
                EwdsEventType::Community,
                if ewds {
                    json!({"communityId": community, "communityName": "Community", "sites": ["Site-A"]})
                } else {
                    json!({"community_id": community, "community_name": "Community", "sites": ["Site-A"]})
                },
            ),
        ] {
            // Event replay and repeated HTTP upserts must not replace mappings.
            for _ in 0..2 {
                if ewds {
                    handle_event(
                        &app.db_wrapper,
                        EwdsEventEnvelope {
                            event_id: format!("topology-{route}"),
                            event_type,
                            occurred_at: "2026-11-18T00:00:00Z".into(),
                            data: json!([data]),
                        },
                    )
                    .await
                    .unwrap();
                } else {
                    let response = http
                        .post(format!("{}/{route}", app.address))
                        .json(&data)
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(
                        response.status().as_u16(),
                        200,
                        "{}",
                        response.text().await.unwrap()
                    );
                }
            }
        }
        let db = &app.db_wrapper;
        for id in ["LIC01", owner, "Site-A", community] {
            let mappings = db
                .ids()
                .filter(Some(derive_onchain_id(id)), None)
                .await
                .unwrap();
            assert_eq!(mappings.len(), 1);
            assert_eq!(mappings[0].offchain_id, id);
        }
        assert_eq!(db.ids().count_documents(doc! {}).await.unwrap(), 4);
        assert_eq!(db.orders().count_documents(doc! {}).await.unwrap(), 0);
        assert!(db
            .ids()
            .filter(None, Some("not-yet-registered".into()))
            .await
            .unwrap()
            .is_empty());

        let mut facility = db.facilities().get_by_name("PV").await.unwrap().unwrap();
        facility.owner_id = "new-owner".into();
        db.facilities().upsert(facility).await.unwrap();
        assert_eq!(
            db.ids()
                .filter(None, Some(owner.into()))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.ids()
                .filter(None, Some("new-owner".into()))
                .await
                .unwrap()
                .len(),
            1
        );
        stop_app(app).await;
    }
}

#[tokio::test]
async fn mapping_failure_does_not_publish_topology_or_replace_existing_mapping() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let existing = IdMappingSchema {
        offchain_id: "owner".into(),
        onchain_id: derive_onchain_id("different-owner"),
        creation_time: 1,
    };
    db.ids().insert_one(&existing).await.unwrap();
    let facility = FacilitySchema {
        facility_id: "LIC01".into(),
        facility_name: "PV".into(),
        site_id: "Site-A".into(),
        owner_id: "owner".into(),
    };
    for insert in [false, true] {
        let error = if insert {
            db.facilities().insert(facility.clone()).await
        } else {
            db.facilities().upsert(facility.clone()).await
        }
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("does not match ACTOR_ID_HASH_KEY"));
    }
    assert_eq!(db.facilities().count_documents(doc! {}).await.unwrap(), 0);
    assert_eq!(
        db.ids().filter(None, Some("owner".into())).await.unwrap(),
        vec![existing]
    );
    // A mapping left by a failed write is not evidence of a registered facility.
    assert_eq!(
        db.ids()
            .filter(None, Some("LIC01".into()))
            .await
            .unwrap()
            .len(),
        1
    );
    stop_app(app).await;
}

#[tokio::test]
async fn uuid_shaped_actor_ids_are_distinct_and_reverse_lookup_preserves_case() {
    let app = init_app().await;
    let upper = "ABCDEFAB-1234-4123-8123-ABCDEFABCDEF";
    let lower = upper.to_lowercase();
    let first = app
        .db_wrapper
        .ids()
        .get_or_create(upper.into())
        .await
        .unwrap();
    let second = app
        .db_wrapper
        .ids()
        .get_or_create(lower.clone())
        .await
        .unwrap();
    assert_ne!(first.onchain_id, second.onchain_id);
    for mapping in [first, second] {
        assert_eq!(
            app.db_wrapper
                .ids()
                .filter(Some(mapping.onchain_id.clone()), None)
                .await
                .unwrap(),
            vec![mapping]
        );
    }
    assert_eq!(
        app.db_wrapper.ids().count_documents(doc! {}).await.unwrap(),
        2
    );
    stop_app(app).await;
}
