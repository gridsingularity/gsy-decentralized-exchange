use launchpad_matching_service::auth::model::{
    MIN_SERVICE_PASSWORD_LENGTH, PasswordError, User, UserModel, hash_password,
};
use mongodb::bson::doc;
use std::time::{SystemTime, UNIX_EPOCH};

async fn setup_db() -> Option<UserModel> {
    UserModel::new().await.ok()
}

fn unique_username(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock is before the Unix epoch")
        .as_nanos();
    format!("{}_{}", prefix, nanos)
}

#[test]
fn test_hash_password_accepts_long_password() {
    let password = "correct-horse-battery-staple";
    assert!(password.chars().count() >= MIN_SERVICE_PASSWORD_LENGTH);

    let hash = hash_password(password).expect("Failed to hash password");
    assert_ne!(hash, password);
    assert!(bcrypt::verify(password, &hash).unwrap());
    assert!(!bcrypt::verify("wrong-password-entirely", &hash).unwrap());
}

#[test]
fn test_hash_password_rejects_short_password() {
    let password = "fifteen-chars-x";
    assert_eq!(password.chars().count(), 15);

    match hash_password(password) {
        Err(PasswordError::TooShort { min }) => assert_eq!(min, MIN_SERVICE_PASSWORD_LENGTH),
        Err(e) => panic!("Unexpected error: {}", e),
        Ok(_) => panic!("Expected a 15 character password to be rejected"),
    }
}

#[tokio::test]
async fn test_upsert_user_inserts_then_updates() {
    let model = match setup_db().await {
        Some(m) => m,
        None => return,
    };
    let collection = model.db.collection::<User>("users");
    let username = unique_username("test_upsert_user");

    let first_hash = hash_password("first-password-0123456789").unwrap();
    model
        .upsert_user(&username, &first_hash)
        .await
        .expect("Failed to insert user");
    let user = model
        .find_by_username(&username)
        .await
        .expect("Failed to find user")
        .expect("User was not inserted");
    assert_eq!(user.password_hash, first_hash);

    let second_hash = hash_password("second-password-0123456789").unwrap();
    model
        .upsert_user(&username, &second_hash)
        .await
        .expect("Failed to update user");
    let user = model
        .find_by_username(&username)
        .await
        .expect("Failed to find user")
        .expect("User disappeared after update");
    assert_eq!(user.username, username);
    assert_eq!(user.password_hash, second_hash);

    let count = collection
        .count_documents(doc! { "username": username.as_str() }, None)
        .await
        .expect("Failed to count users");
    assert_eq!(count, 1);

    collection
        .delete_many(doc! { "username": username.as_str() }, None)
        .await
        .expect("Failed to clean up test user");
}
