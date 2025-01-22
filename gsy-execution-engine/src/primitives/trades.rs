use serde::Deserialize;

/// TODO: Change with the offchain storage returned struct
#[derive(Debug, Deserialize)]
pub struct Trade {
    pub area_uuid: String,
    pub energy: f32,
    pub price: f32,
    pub time_slot: String,
    // ...
}
