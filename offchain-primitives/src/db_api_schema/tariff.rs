//! Tariff schema, as specified in D3.2 §5.4.
//!
//! Tariffs are persisted alongside the order book so that price
//! components applied to each asset (energy price, grid fee, taxes,
//! incentives) are reachable in the same storage that produces the
//! orders they apply to.

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
