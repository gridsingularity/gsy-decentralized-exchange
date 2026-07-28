use gsy_offchain_storage::db::Coll;
use mongodb::bson::doc;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Item {
    key: String,
    value: i64,
}

#[tokio::test]
async fn replace_one_upsert_replaces_when_matched() {
    let store = Arc::new(RwLock::new(vec![
        Item {
            key: "a".to_string(),
            value: 1,
        },
        Item {
            key: "b".to_string(),
            value: 2,
        },
    ]));
    let coll: Coll<Item> = Coll::InMemory(store);

    let replacement = Item {
        key: "a".to_string(),
        value: 99,
    };
    let summary = coll
        .replace_one_upsert(doc! {"key": "a"}, replacement.clone(), |item| {
            item.key == "a"
        })
        .await
        .unwrap();

    assert_eq!(summary.matched_count, 1);
    assert_eq!(summary.modified_count, 1);
    let all = coll.all().await.unwrap();
    assert_eq!(all.len(), 2, "replacing must not add a new item");
    assert!(all.contains(&replacement));
}

#[tokio::test]
async fn replace_one_upsert_pushes_when_unmatched() {
    let store = Arc::new(RwLock::new(vec![Item {
        key: "a".to_string(),
        value: 1,
    }]));
    let coll: Coll<Item> = Coll::InMemory(store);

    let new_item = Item {
        key: "c".to_string(),
        value: 3,
    };
    let summary = coll
        .replace_one_upsert(doc! {"key": "c"}, new_item.clone(), |item| item.key == "c")
        .await
        .unwrap();

    assert_eq!(summary.matched_count, 0);
    assert_eq!(summary.modified_count, 1);
    let all = coll.all().await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&new_item));
}
