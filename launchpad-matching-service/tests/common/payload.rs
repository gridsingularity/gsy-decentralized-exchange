//! `POST /match` request bodies of a given size, for the benchmark and the
//! body limit tests.

use super::rng::Rng;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbBid, DbOffer, DbOrderComponent, DbOrderSchema, Order,
};
use launchpad_matching_service::api::types::OrdersToMatch;

/// Quarter-hour slots in a day.
pub const SLOTS_PER_DAY: u64 = 96;
/// Realistic shape: orders per (market, slot) before a new market starts.
pub const ORDERS_PER_GROUP: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BatchShape {
    /// The orders fill the 96 quarter-hour slots of a day round robin; every
    /// `96 * ORDERS_PER_GROUP` orders a new market starts.
    Realistic,
    /// Every order in one market and one slot.
    WorstCase,
    /// Worst case for the optimized matching: one market, one slot and one
    /// area, so no pair may match and every bid / offer pair is visited.
    SingleArea,
}

impl BatchShape {
    pub fn name(&self) -> &'static str {
        match self {
            BatchShape::Realistic => "realistic",
            BatchShape::WorstCase => "worst case",
            BatchShape::SingleArea => "single area",
        }
    }
}

fn uuid_like(rng: &mut Rng) -> String {
    let a = rng.next_u64();
    let b = rng.next_u64();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xffff,
        a & 0xfff,
        (b >> 48) | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

/// SS58-like account (48 characters).
fn account_like(rng: &mut Rng) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut s = String::with_capacity(48);
    s.push('5');
    for _ in 1..48 {
        s.push(*rng.pick(ALPHABET) as char);
    }
    s
}

/// Order `index` of a batch with this shape.
fn order(rng: &mut Rng, shape: BatchShape, index: u64, markets: &mut Vec<String>) -> DbOrderSchema {
    let (market, slot) = match shape {
        BatchShape::Realistic => (
            (index / SLOTS_PER_DAY / ORDERS_PER_GROUP) as usize,
            index % SLOTS_PER_DAY,
        ),
        BatchShape::WorstCase | BatchShape::SingleArea => (0, 0),
    };
    while markets.len() <= market {
        markets.push(uuid_like(rng));
    }
    // 500 households; buyers and sellers rarely share an area.
    let household = match shape {
        BatchShape::SingleArea => 0,
        _ => rng.below(500),
    };
    let area = format!("{:08x}-0000-4000-8000-{:012x}", household, 0);
    let is_bid = rng.chance(0.5);
    let component = DbOrderComponent {
        area_uuid: area,
        market_id: markets[market].clone(),
        time_slot: 1_767_225_600 + 900 * slot,
        creation_time: 1_767_139_200 + rng.below(86_400),
        energy: (rng.range(0.05, 5.0) * 1000.0).round() / 1000.0,
        // Bids 15 to 35, offers 5 to 30 (cents / kWh): mostly overlapping.
        energy_rate: if is_bid {
            (rng.range(15.0, 35.0) * 100.0).round() / 100.0
        } else {
            (rng.range(5.0, 30.0) * 100.0).round() / 100.0
        },
    };
    let nonce = rng.below(1_000) as u32;
    let order = if is_bid {
        Order::Bid(DbBid {
            buyer: account_like(rng),
            nonce,
            bid_component: component,
        })
    } else {
        Order::Offer(DbOffer {
            seller: account_like(rng),
            nonce,
            offer_component: component,
        })
    };
    DbOrderSchema {
        _id: format!(
            "{:024x}",
            0x6717_0000_0000_0000_0000_0000u128 + index as u128
        ),
        status: Default::default(),
        order,
    }
}

/// The largest batch of this shape whose JSON body is at most
/// `max_bytes`, with its body.
pub fn batch_of_size(shape: BatchShape, max_bytes: usize, seed: u64) -> (OrdersToMatch, Vec<u8>) {
    let mut rng = Rng::new(seed);
    let mut markets = Vec::new();
    let mut batch = OrdersToMatch {
        user_id: "6717a8c2e4b0f1a2b3c4d5e6".to_string(),
        orders: Vec::new(),
    };
    // `{"user_id":"...","orders":[` + orders joined by `,` + `]}`.
    let mut size = serde_json::to_vec(&batch).unwrap().len();
    loop {
        let next = order(&mut rng, shape, batch.orders.len() as u64, &mut markets);
        let order_size = serde_json::to_vec(&next).unwrap().len();
        let separator = usize::from(!batch.orders.is_empty());
        if size + separator + order_size > max_bytes {
            break;
        }
        size += separator + order_size;
        batch.orders.push(next);
    }
    let body = serde_json::to_vec(&batch).unwrap();
    assert_eq!(body.len(), size, "size bookkeeping is off");
    (batch, body)
}
