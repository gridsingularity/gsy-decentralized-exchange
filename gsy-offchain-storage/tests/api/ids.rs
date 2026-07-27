// tests/id_service.rs  (or src/db/id_mapping.rs `#[cfg(test)] mod tests`)
use crate::helpers::{init_app, stop_app, TestApp};
use actix_web::web;
use anyhow::Result;
use futures::StreamExt;
use gsy_offchain_storage::db::id_service::{init_id_mapping, IdService};
use gsy_offchain_storage::db::DatabaseWrapper;
use mongodb::bson::doc;
use primitives::db_api_schema::ids::IdType;

/// Spins up a wrapper against a throwaway database.
pub async fn spawn_app_with_db() -> (TestApp, web::Data<DatabaseWrapper>) {
    let app = init_app().await;
    let db = web::Data::new(app.db_wrapper.clone());
    (app, db)
}

#[tokio::test]
async fn get_or_create_inserts_when_absent() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let stored = service
        .get_or_create("offchain-a".to_string(), "actor_id".to_string())
        .await?;

    assert_eq!(stored.offchain_id, "offchain-a");
    assert_eq!(stored.id_type, IdType::ActorId);
    assert!(!stored.onchain_id.is_empty());
    assert_eq!(service.count_documents(doc! {}).await?, 1);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn get_or_create_is_idempotent() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let first = service
        .get_or_create("offchain-b".to_string(), "order_id".to_string())
        .await?;

    // Same (offchain_id, id_type) — must return the identical row, no new insert.
    let second = service
        .get_or_create("offchain-b".to_string(), "order_id".to_string())
        .await?;

    assert_eq!(first, second);
    assert_eq!(second.id_type, IdType::OrderId);
    assert_eq!(service.count_documents(doc! {}).await?, 1);

    // Different id_type for the same offchain_id — distinct identity, new row.
    let third = service
        .get_or_create("offchain-b".to_string(), "trade_id".to_string())
        .await?;

    assert_ne!(third, first);
    assert_eq!(third.id_type, IdType::TradeId);
    assert_eq!(third.offchain_id, first.offchain_id);
    assert_eq!(service.count_documents(doc! {}).await?, 2);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn get_or_create_is_deterministic() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let a = service
        .get_or_create("same-offchain".to_string(), "actor_id".to_string())
        .await?;
    let b = service
        .get_or_create("same-offchain".to_string(), "actor_id".to_string())
        .await?;

    // Same offchain_id encrypts to the same onchain_id.
    assert_eq!(a.onchain_id, b.onchain_id);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_by_onchain_id() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let a = service
        .get_or_create("off-1".to_string(), "actor_id".to_string())
        .await?;
    service
        .get_or_create("off-2".to_string(), "market_id".to_string())
        .await?;

    let found = service.filter(Some(a.onchain_id.clone()), None, None).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].offchain_id, "off-1");

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_by_offchain_id_and_type() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    service
        .get_or_create("off-3".to_string(), "trade_id".to_string())
        .await?;

    let hit = service
        .filter(None, Some("off-3".into()), Some("trade_id".into()))
        .await?;
    assert_eq!(hit.len(), 1);

    // Type mismatch narrows to nothing.
    let miss = service
        .filter(None, Some("off-3".into()), Some("market_id".into()))
        .await?;
    assert!(miss.is_empty());

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_by_type_only_returns_all_of_that_type() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    service
        .get_or_create("off-4".to_string(), "actor_id".to_string())
        .await?;
    service
        .get_or_create("off-5".to_string(), "actor_id".to_string())
        .await?;
    service
        .get_or_create("off-6".to_string(), "market_id".to_string())
        .await?;

    let actors = service.filter(None, None, Some("actor_id".into())).await?;
    assert_eq!(actors.len(), 2);

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_rejects_mutually_exclusive_ids() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    let service = IdService::from(&**db);

    let err = service
        .filter(Some("0x777".into()), Some("off-7".into()), None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("mutually exclusive"));

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_rejects_empty_params() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    let service = IdService::from(&**db);

    let err = service.filter(None, None, None).await.unwrap_err();
    assert!(err.to_string().contains("at least one filter field"));

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn filter_returns_empty_for_unknown_id() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let found = service.filter(Some("0xnope".into()), None, None).await?;
    assert!(found.is_empty());

    stop_app(app).await;
    Ok(())
}

#[tokio::test]
async fn init_creates_both_indexes() -> Result<()> {
    let (app, db) = spawn_app_with_db().await;
    init_id_mapping(&db).await?;
    let service = IdService::from(&**db);

    let mut names = Vec::new();
    let mut cursor = service.list_indexes().await?;
    while let Some(idx) = cursor.next().await {
        if let Some(keys) = idx?.keys.keys().next() {
            names.push(keys.to_string());
        }
    }
    assert!(names.contains(&"onchain_id".to_string()));
    assert!(names.contains(&"offchain_id".to_string()));

    stop_app(app).await;
    Ok(())
}