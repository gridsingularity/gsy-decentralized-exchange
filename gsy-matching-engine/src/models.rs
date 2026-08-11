use primitives::db_api_schema::orders::{IntelligentEnergyType, OrderEnum, OrderStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct Requirements {
    pub trading_partner_id: Option<String>,
    pub energy_type: Option<IntelligentEnergyType>,
    pub preferred_energy_rate: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attributes {
    pub trading_partner_id: Option<String>,
    pub energy_type: IntelligentEnergyType,
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
    bids: Vec<Order>,
    offers: Vec<Order>,
    market_id: String,
    time_slot: u64,
}

impl MatchingData {
    pub fn new(
        market_id: String,
        time_slot: u64,
        bids: Vec<Order>,
        offers: Vec<Order>,
    ) -> Result<Self, String> {
        let mismatched_order = bids
            .iter()
            .chain(offers.iter())
            .find(|order| order.market_id != market_id || order.time_slot != time_slot);

        if let Some(order) = mismatched_order {
            return Err(format!(
                "Order {} belongs to market {} timeslot {}, expected market {} timeslot {}",
                order.order_id, order.market_id, order.time_slot, market_id, time_slot
            ));
        }

        Ok(Self {
            bids,
            offers,
            market_id,
            time_slot,
        })
    }

    pub fn bids(&self) -> &[Order] {
        &self.bids
    }

    pub fn offers(&self) -> &[Order] {
        &self.offers
    }

    pub fn market_id(&self) -> &str {
        &self.market_id
    }

    pub fn time_slot(&self) -> u64 {
        self.time_slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(order_id: &str, market_id: &str, time_slot: u64) -> Order {
        Order {
            order_id: order_id.to_string(),
            order_type: OrderEnum::Bid,
            status: OrderStatus::Open,
            area_uuid: "area".to_string(),
            market_id: market_id.to_string(),
            time_slot,
            creation_time: 1,
            energy: 1,
            energy_rate: 1,
            created_by: "actor".to_string(),
            requirements: None,
            attributes: None,
        }
    }

    #[test]
    fn rejects_orders_from_another_market_slot() {
        let result = MatchingData::new(
            "market-a".to_string(),
            100,
            vec![order("bid", "market-b", 100)],
            Vec::new(),
        );

        assert!(result.is_err());
    }
}
