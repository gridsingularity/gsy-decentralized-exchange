use ethers::types::H256;
use mongodb::{options::ClientOptions, Database};

pub async fn test_database() -> Database {
    let db_url = std::env::var("MONGO_URL").unwrap_or_else(|_| {
        "mongodb://gsy:gsy@mongodb:27017/?retryWrites=true&w=majority".to_string()
    });
    let db_name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "offchain_storage".to_string());
    let options = ClientOptions::parse(&db_url)
        .await
        .expect("Invalid MONGO_URL");
    mongodb::Client::with_options(options)
        .expect("Failed to create the Mongo client")
        .database(db_name.as_str())
}

/// Solidity encodes indexed bytes16 values by right-padding them to 32 bytes.
pub fn indexed_bytes16_topic(id: [u8; 16]) -> H256 {
    let mut topic = [0u8; 32];
    topic[..16].copy_from_slice(&id);
    H256::from(topic)
}
