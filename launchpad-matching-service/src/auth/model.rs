use crate::configuration::get_configuration;
use mongodb::{Client, Collection, bson::doc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub username: String,
    pub password_hash: String,
}

pub struct UserModel {
    db: mongodb::Database,
}

impl UserModel {
    pub async fn new() -> mongodb::error::Result<Self> {
        let mongodb_uri = get_configuration().unwrap().get_connection_string();
        let client = Client::with_uri_str(mongodb_uri).await?;
        let db = client.database("launchpad");
        Ok(UserModel { db })
    }

    pub async fn find_by_username(
        &self,
        username: &str,
    ) -> mongodb::error::Result<Option<User>> {
        let collection: Collection<User> = self.db.collection("users");
        let filter = doc! { "username": username };
        collection.find_one(filter, None).await
    }
}
