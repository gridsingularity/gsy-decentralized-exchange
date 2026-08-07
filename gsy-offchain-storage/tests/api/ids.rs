use anyhow::Result;
use gsy_offchain_storage::db::id_service::{init_id_mapping, IdService};
use crate::helpers::{init_app, stop_app};
use mongodb::bson::doc;
use primitives::utils::{bytes16_to_hex, create_encrypted_bytes16_from_string};

// --- Pure-logic tests (no DB) ----------------------------------------------

#[test]
fn onchain_id_is_deterministic_hex_form() {
    let offchain = "test-offchain-id";
    let a = bytes16_to_hex(create_encrypted_bytes16_from_string(offchain));
    let b = bytes16_to_hex(create_encrypted_bytes16_from_string(offchain));
    assert_eq!(a, b, "hashing must be deterministic for the same input");
    assert!(a.starts_with("0x"), "onchain id must be 0x-prefixed hex");
    assert_eq!(a.len(), 34, "0x + 32 hex chars for 16 bytes");
}

#[test]
fn onchain_id_differs_for_different_inputs() {
    let a = bytes16_to_hex(create_encrypted_bytes16_from_string("id-a"));
    let b = bytes16_to_hex(create_encrypted_bytes16_from_string("id-b"));
    assert_ne!(a, b);
}

// --- DB-backed tests --------------------------------------------------------

#[tokio::test]
async fn get_or_create_inserts_new_mapping() -> Result<()> {
    let app = init_app().await;
    init_id_mapping(&app.db_wrapper).await?;
    let service = IdService::from(&app.db_wrapper);

    let offchain = "actor-123".to_string();
    let created = service.get_or_create(offchain.clone()).await?;

    assert_eq!(created.offchain_id, offchain);
    let expected = bytes16_to_hex(create_encrypted_bytes16_from_string(&offchain));
    assert_eq!(created.onchain_id, expected);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn get_or_create_is_idempotent() -> Result<()> {
    let app = init_app().await;
    init_id_mapping(&app.db_wrapper).await?;
    let service = IdService::from(&app.db_wrapper);

    let offchain = "actor-456".to_string();
    let first = service.get_or_create(offchain.clone()).await?;
    let second = service.get_or_create(offchain.clone()).await?;

    // Same onchain id and creation_time => no re-insert on second call.
    assert_eq!(first.onchain_id, second.onchain_id);
    assert_eq!(first.creation_time, second.creation_time);

    let count = service
        .count_documents(doc! {"offchain_id": &offchain})
        .await?;
    assert_eq!(count, 1, "upsert must not create duplicates");

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_by_offchain_id_returns_match() -> Result<()> {
    let app = init_app().await;
    init_id_mapping(&app.db_wrapper).await?;
    let service = IdService::from(&app.db_wrapper);

    let offchain = "actor-789".to_string();
    service.get_or_create(offchain.clone()).await?;

    let results = service.filter(None, Some(offchain.clone())).await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].offchain_id, offchain);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_by_onchain_id_returns_match() -> Result<()> {
    let app = init_app().await;
    init_id_mapping(&app.db_wrapper).await?;
    let service = IdService::from(&app.db_wrapper);

    let offchain = "actor-abc".to_string();
    let created = service.get_or_create(offchain.clone()).await?;

    let results = service.filter(Some(created.onchain_id.clone()), None).await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].onchain_id, created.onchain_id);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_no_match_returns_empty() -> Result<()> {
    let app = init_app().await;
    init_id_mapping(&app.db_wrapper).await?;
    let service = IdService::from(&app.db_wrapper);

    let results = service.filter(None, Some("does-not-exist".to_string())).await?;

    assert!(results.is_empty());

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_rejects_both_ids() {
    let app = init_app().await;
    let service = IdService::from(&app.db_wrapper);

    let err = service
        .filter(Some("a".to_string()), Some("b".to_string()))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("mutually exclusive"));

    stop_app(app).await;
}

#[tokio::test]
async fn filter_rejects_empty() {
    let app = init_app().await;
    let service = IdService::from(&app.db_wrapper);

    let err = service.filter(None, None).await.unwrap_err();
    assert!(err.to_string().contains("at least one filter field"));

    stop_app(app).await;
}