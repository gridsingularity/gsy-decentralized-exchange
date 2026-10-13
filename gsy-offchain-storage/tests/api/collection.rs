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

#[tokio::test]
async fn insert_if_absent_inserts_only_when_unmatched() {
    let stored = Item {
        key: "a".to_string(),
        value: 1,
    };
    let coll: Coll<Item> = Coll::InMemory(Arc::new(RwLock::new(vec![stored.clone()])));

    let inserted = coll
        .insert_if_absent(
            doc! {"key": "a"},
            Item {
                key: "a".to_string(),
                value: 2,
            },
            |item| item.key == "a",
        )
        .await
        .unwrap();
    assert!(!inserted);
    assert_eq!(coll.all().await.unwrap(), vec![stored.clone()]);

    let new_item = Item {
        key: "b".to_string(),
        value: 3,
    };
    let inserted = coll
        .insert_if_absent(doc! {"key": "b"}, new_item.clone(), |item| item.key == "b")
        .await
        .unwrap();
    assert!(inserted);
    assert_eq!(coll.all().await.unwrap(), vec![stored, new_item]);
}

#[tokio::test]
async fn remove_duplicates_by_keeps_the_preferred_item_of_each_key() {
    let item = |key: &str, value: i64| Item {
        key: key.to_string(),
        value,
    };
    let coll: Coll<Item> = Coll::InMemory(Arc::new(RwLock::new(vec![
        item("a", 1),
        item("b", 5),
        item("a", 3),
        item("c", 7),
        item("a", 3),
        item("b", 2),
    ])));

    // Prefer the greatest value; of equal ones the earlier item stays.
    let removed = coll
        .remove_duplicates_by(
            &["key"],
            vec![doc! {"$sort": {"value": -1}}],
            |item| item.key.clone(),
            |a, b| b.value.cmp(&a.value),
        )
        .await
        .unwrap();

    assert_eq!(removed, 3);
    assert_eq!(
        coll.all().await.unwrap(),
        vec![item("b", 5), item("a", 3), item("c", 7)],
        "survivors keep their stored order"
    );
}
