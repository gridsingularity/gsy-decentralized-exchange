use primitives::db_api_schema::orders::{EnergyType, OrderEnum, OrderStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct Requirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<EnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: EnergyType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub order_id: String,
    pub order_type: OrderEnum,
    pub status: OrderStatus,
    pub area_uuid: String,
    pub market_id: String,
    pub time_slot: u64,
    pub creation_time: u64,
    pub energy: u64,
    pub energy_rate: u64,
    pub created_by: String,
    pub requirements: Option<Requirements>,
    pub attributes: Option<Attributes>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BidOfferMatch {
    pub market_id: String,
    pub time_slot: u64,
    pub bid: Order,
    pub offer: Order,
    pub residual_bid: Option<Order>,
    pub residual_offer: Option<Order>,
    pub selected_energy: u64,
    pub energy_rate: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchingData {
    pub bids: Vec<Order>,
    pub offers: Vec<Order>,
    pub market_id: String,
}
