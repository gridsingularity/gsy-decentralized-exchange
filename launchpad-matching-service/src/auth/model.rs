use crate::configuration::get_configuration;
use mongodb::{Client, Collection, bson::doc, options::UpdateOptions};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Minimum password length (in characters) accepted for service accounts.
pub const MIN_SERVICE_PASSWORD_LENGTH: usize = 16;

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub username: String,
    pub password_hash: String,
}

/// Errors returned by `hash_password`. Messages never include the password.
#[derive(Debug)]
pub enum PasswordError {
    TooShort { min: usize },
    Hash(bcrypt::BcryptError),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PasswordError::TooShort { min } => {
                write!(f, "password is too short (minimum {} characters)", min)
            }
            PasswordError::Hash(e) => write!(f, "failed to hash password: {}", e),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Hashes a service account password with bcrypt after checking its length.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    if password.chars().count() < MIN_SERVICE_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort {
            min: MIN_SERVICE_PASSWORD_LENGTH,
        });
    }
    bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(PasswordError::Hash)
}

pub struct UserModel {
    pub db: mongodb::Database,
}

impl UserModel {
    pub async fn new() -> mongodb::error::Result<Self> {
        let config = get_configuration().unwrap();
        let mongodb_uri = config.get_connection_string();
        let client = Client::with_uri_str(mongodb_uri).await?;
        let db = client.database(config.database_name.as_str());
        Ok(UserModel::from_database(db))
    }

    /// Builds a model on an existing database handle (shared `Client`).
    pub fn from_database(db: mongodb::Database) -> Self {
        UserModel { db }
    }

    pub async fn find_by_username(&self, username: &str) -> mongodb::error::Result<Option<User>> {
        let collection: Collection<User> = self.db.collection("users");
        let filter = doc! { "username": username };
        collection.find_one(filter, None).await
    }

    /// Creates the user, or replaces its password hash if it already exists.
    pub async fn upsert_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> mongodb::error::Result<()> {
        let collection: Collection<User> = self.db.collection("users");
        let filter = doc! { "username": username };
        let update = doc! { "$set": { "password_hash": password_hash } };
        let options = UpdateOptions::builder().upsert(true).build();
        collection.update_one(filter, update, options).await?;
        Ok(())
    }
}
