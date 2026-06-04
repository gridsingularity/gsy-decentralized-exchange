use codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone, PartialEq)]
pub struct TariffSchema {
    pub tariff_name: String,
    pub tariff_structure: String,
    pub energy_price: f64,
    pub grid_fee: f64,
    pub taxes: f64,
    pub incentives: f64,
    pub currency: String,
    pub tariff_start: String,
    pub tariff_end: String,
}
