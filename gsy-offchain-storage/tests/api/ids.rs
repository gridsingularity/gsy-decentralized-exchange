use crate::helpers::{init_app, stop_app};
use gsy_offchain_storage::db::id_service::init_ids;
use mongodb::bson::doc;
use primitives::db_api_schema::ids::IdMappingSchema;
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};

#[tokio::test]
async fn filter_by_onchain_id_returns_original_facility_id() {
    let app = init_app().await;
    let facility_id = "00112233-4455-6677-8899-aabbccddeeff";
    let mapping = app
        .db_wrapper
        .ids()
        .get_or_create(facility_id.to_string())
        .await
        .unwrap();
    let recovered = app
        .db_wrapper
        .ids()
        .filter(Some(mapping.onchain_id), None)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].offchain_id, facility_id);
    assert!(app
        .db_wrapper
        .ids()
        .filter(Some("0xffffffffffffffffffffffffffffffff".to_string()), None)
        .await
        .unwrap()
        .is_empty());
    stop_app(app).await;
}

#[tokio::test]
async fn post_ids_creates_new_mapping() {
    let app = init_app().await;
    init_ids(&app.db_wrapper)
        .await
        .expect("Failed to init id mapping indexes");
    let client = reqwest::Client::new();
    let offchain = "actor-123";

    let response = client
        .post(format!("{}/ids", app.address))
        .query(&[("offchain_id", offchain)])
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 200);

    let body: IdMappingSchema = response.json().await.expect("Failed to parse body");
    assert_eq!(body.offchain_id, offchain);
    let expected = bytes16_to_hex(create_encrypted_bytes16_from_string(offchain));
    assert_eq!(body.onchain_id, expected);

    stop_app(app).await;
}

#[tokio::test]
async fn post_ids_is_idempotent() {
    let app = init_app().await;
    init_ids(&app.db_wrapper)
        .await
        .expect("Failed to init id mapping indexes");
    let client = reqwest::Client::new();
    let offchain = "actor-456";

    let first: IdMappingSchema = client
        .post(format!("{}/ids", app.address))
        .query(&[("offchain_id", offchain)])
        .send()
        .await
        .expect("Failed to execute first request")
        .json()
        .await
        .expect("Failed to parse first body");

    let second: IdMappingSchema = client
        .post(format!("{}/ids", app.address))
        .query(&[("offchain_id", offchain)])
        .send()
        .await
        .expect("Failed to execute second request")
        .json()
        .await
        .expect("Failed to parse second body");

    // Same mapping returned; the unique offchain_id index plus $setOnInsert
    // means the second call reads the existing doc rather than inserting again.
    assert_eq!(first.onchain_id, second.onchain_id);
    assert_eq!(first.creation_time, second.creation_time);

    // Prove no duplicate slipped in under the unique index.
    let count = app
        .db_wrapper
        .ids()
        .count_documents(doc! {"offchain_id": offchain})
        .await
        .expect("Failed to count documents");
    assert_eq!(count, 1, "unique index must prevent duplicate mappings");

    stop_app(app).await;
}

#[tokio::test]
async fn post_ids_distinct_inputs_get_distinct_onchain_ids() {
    let app = init_app().await;
    init_ids(&app.db_wrapper)
        .await
        .expect("Failed to init id mapping indexes");
    let client = reqwest::Client::new();

    let a: IdMappingSchema = client
        .post(format!("{}/ids", app.address))
        .query(&[("offchain_id", "actor-a")])
        .send()
        .await
        .expect("Failed to execute request a")
        .json()
        .await
        .expect("Failed to parse body a");

    let b: IdMappingSchema = client
        .post(format!("{}/ids", app.address))
        .query(&[("offchain_id", "actor-b")])
        .send()
        .await
        .expect("Failed to execute request b")
        .json()
        .await
        .expect("Failed to parse body b");

    assert_ne!(a.onchain_id, b.onchain_id);

    stop_app(app).await;
}

#[tokio::test]
async fn post_ids_concurrent_same_id_yields_single_mapping() {
    let app = init_app().await;
    init_ids(&app.db_wrapper)
        .await
        .expect("Failed to init id mapping indexes");
    let client = reqwest::Client::new();
    let offchain = "actor-concurrent";
    let url = format!("{}/ids", app.address);

    // Fire several requests for the same offchain_id at once. The unique index
    // is what keeps this from producing duplicate documents under the race.
    let mut handles = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client
                .post(&url)
                .query(&[("offchain_id", offchain)])
                .send()
                .await
                .expect("Failed to execute concurrent request")
                .json::<IdMappingSchema>()
                .await
                .expect("Failed to parse concurrent body")
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.expect("Task panicked"));
    }

    // Every response must describe the same mapping.
    let onchain = &results[0].onchain_id;
    for r in &results {
        assert_eq!(&r.onchain_id, onchain);
    }

    // And exactly one document exists in the collection.
    let count = app
        .db_wrapper
        .ids()
        .count_documents(doc! {"offchain_id": offchain})
        .await
        .expect("Failed to count documents");
    assert_eq!(count, 1, "concurrent inserts must collapse to one mapping");

    stop_app(app).await;
}

#[tokio::test]
async fn post_ids_missing_param_is_rejected() {
    let app = init_app().await;
    init_ids(&app.db_wrapper)
        .await
        .expect("Failed to init id mapping indexes");
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/ids", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // Query<IdsParams> extraction fails when offchain_id is absent -> 400.
    assert_eq!(response.status().as_u16(), 400);

    stop_app(app).await;
}
