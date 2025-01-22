use serde::Deserialize;

/// TODO: Change with the offchain storage returned struct
#[derive(Debug, Deserialize)]
pub struct Measurement {
    pub area_uuid: String,
    pub energy: f32,
    pub timestamp: String,
    // ...
}