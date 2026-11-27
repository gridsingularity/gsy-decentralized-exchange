use chrono::DateTime;
use crate::helpers::{init_app, stop_app, ts};
use gsy_offchain_storage::ewds_event_handler::handle_event;
use mongodb::bson::doc;
use primitives::{
    db_api_schema::{
        grid_topology::{EnergyCommunitySchema, FacilitySchema},
        ids::IdMappingSchema,
    },
    ewds::{dto::EwdsEventEnvelope, EwdsEventType},
    utils::derive_onchain_id,
};
use serde_json::json;

#[tokio::test]
async fn topology_registers_actor_mappings_over_http_and_ewds_before_orders_exist() {
    let community = "Community-MiXeD/LIC01";
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
                    "site_id": "Site-A", "site_name": "Site display name", "site_description": "Site", "facilities": ["LIC01", "not-yet-registered"]
                }),
            ),
            (
                "communities",
                EwdsEventType::Community,
                json!({"communityId": community, "communityName": "Community", "sites": ["Site-A"]}),
            ),
        ] {
            // Event replay and repeated HTTP writes must not replace mappings or add versions.
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
        assert_eq!(db.facilities().count_documents(doc! {}).await.unwrap(), 1);
        assert_eq!(db.sites().count_documents(doc! {}).await.unwrap(), 1);
        assert!(db
            .ids()
            .filter(None, Some("Site display name".into()))
            .await
            .unwrap()
            .is_empty());
        assert_eq!(db.orders().count_documents(doc! {}).await.unwrap(), 0);
        assert!(db
            .ids()
            .filter(None, Some("not-yet-registered".into()))
            .await
            .unwrap()
            .is_empty());

        let mut facility = db
            .facilities()
            .get_by_ids(&["LIC01"])
            .await
            .unwrap()
            .remove(0);
        facility.owner_id = "new-owner".into();
        db.facilities().store(vec![facility.clone()]).await.unwrap();
        let client = primitives::offchain_storage::OffchainStorageClient::new(
            primitives::offchain_storage::OffchainStorageTransport::Http,
            app.address.clone(),
            "UNUSED",
            "unused",
        );
        assert_eq!(
            client.fetch_facility_owner_mapping().await.unwrap(),
            std::collections::HashMap::from([("LIC01".into(), "new-owner".into())])
        );
        assert_eq!(db.facilities().count_documents(doc! {}).await.unwrap(), 2);
        assert_eq!(
            db.facilities().get().await.unwrap()[0].owner_id,
            "new-owner"
        );
        facility.valid_until = Some(DateTime::UNIX_EPOCH);
        db.facilities().store(vec![facility]).await.unwrap();
        assert!(db.facilities().get().await.unwrap().is_empty());
        assert!(client
            .fetch_facility_owner_mapping()
            .await
            .unwrap()
            .is_empty());
        assert_eq!(db.facilities().count_documents(doc! {}).await.unwrap(), 3);
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
async fn invalid_topology_batch_registers_neither_ids_nor_versions() {
    for event_type in [EwdsEventType::Site, EwdsEventType::Community] {
        let app = init_app().await;
        let data = if event_type == EwdsEventType::Site {
            json!([
                {"site_id": "valid", "site_name": "Valid", "site_description": "", "facilities": []},
                {"site_id": " invalid", "site_name": "Invalid", "site_description": "", "facilities": []}
            ])
        } else {
            json!([
                {"communityId": "valid", "communityName": "Valid", "sites": []},
                {"communityId": "invalid-reference", "communityName": "Invalid", "sites": [" padded "]}
            ])
        };
        let error = handle_event(
            &app.db_wrapper,
            EwdsEventEnvelope {
                event_id: "invalid-topology".into(),
                event_type,
                occurred_at: "2026-11-19T00:00:00Z".into(),
                data,
            },
        )
        .await
        .unwrap_err();
        assert!(primitives::ewds::is_invalid_event(&error));
        assert_eq!(
            app.db_wrapper.ids().count_documents(doc! {}).await.unwrap(),
            0
        );
        assert_eq!(
            app.db_wrapper
                .sites()
                .count_documents(doc! {})
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            app.db_wrapper
                .communities()
                .count_documents(doc! {})
                .await
                .unwrap(),
            0
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
        creation_time: ts(1),
    };
    db.ids().insert_one(&existing).await.unwrap();
    let facility = FacilitySchema {
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
        facility_id: "LIC01".into(),
        facility_name: "PV".into(),
        site_id: "Site-A".into(),
        owner_id: "owner".into(),
    };
    for _ in 0..2 {
        let error = db
            .facilities()
            .store(vec![facility.clone()])
            .await
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
async fn community_mapping_must_succeed_before_topology_is_written() {
    let app = init_app().await;
    let db = &app.db_wrapper;
    let community = EnergyCommunitySchema {
        valid_from: DateTime::UNIX_EPOCH,
        valid_until: None,
        community_id: "Community-MiXeD/LIC01".into(),
        community_name: "Community".into(),
        sites: vec![],
    };
    let wrong_mapping = IdMappingSchema {
        offchain_id: community.community_id.clone(),
        onchain_id: derive_onchain_id("different-community"),
        creation_time: ts(1),
    };
    db.ids().insert_one(&wrong_mapping).await.unwrap();
    for _ in 0..2 {
        let error = db
            .communities()
            .store(vec![community.clone()])
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("does not match ACTOR_ID_HASH_KEY"));
    }
    assert!(db.communities().get().await.unwrap().is_empty());
    assert_eq!(
        db.ids()
            .filter(None, Some(community.community_id))
            .await
            .unwrap(),
        vec![wrong_mapping]
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
