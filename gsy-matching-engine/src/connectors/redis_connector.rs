use crate::algorithms::PayAsBid;
use crate::primitives::web2::{Bid, BidOfferMatch, MatchingData, Offer};

use anyhow::{Error, Result};
use serde_json::{Value, json};
use chrono::{NaiveDateTime};
use redis::Commands;

pub fn value_to_str(value: &Value) -> String {
    // Helper function to convert the serde Value to String
    match value.as_str() {
        Some(..) => value.as_str().unwrap().to_string(),
        None => String::new(),
    }
}

pub fn value_to_f32(value: &Value) -> f32 {
    // Helper function to convert the serde Value to f32
    match value.as_f64() {
        Some(..) => value.as_f64().unwrap() as f32,
        None => 0 as f32,
    }
}

pub fn value_to_datetime(value: &Value) -> Option<NaiveDateTime> {
    // Helper function to convert the serde Value to NaiveDateTime
    match value.as_str() {
        Some(..) => match NaiveDateTime::parse_from_str(value.as_str().unwrap(), "%Y-%m-%dT%H:%M:%S") {
            Ok(datetime) => Some(datetime),
            Err(_e) => None,
        },
        None => None,
    }
}

pub fn read_bids(orders: &Value) -> Vec<Bid> {
    // Create an array of Bid structs from the serde Value
    let mut bids_list = Vec::new();
    for bid in orders.as_array().unwrap() {
        let bid_struct = Bid {
            r#type: value_to_str(&bid["type"]),
            id: value_to_str(&bid["id"]),
            energy: value_to_f32(&bid["energy"]),
            energy_rate: value_to_f32(&bid["energy_rate"]),
            original_price: value_to_f32(&bid["original_price"]),
            requirements: Some(value_to_str(&bid["requirements"])),
            buyer_origin: value_to_str(&bid["buyer_origin"]),
            buyer_origin_id: value_to_str(&bid["buyer_origin_id"]),
            buyer_id: value_to_str(&bid["buyer_id"]),
            buyer: value_to_str(&bid["buyer"]),
            time_slot: value_to_datetime(&bid["time_slot"]),
            creation_time: value_to_datetime(&bid["creation_time"]),
        };
        bids_list.push(bid_struct);
    }
    bids_list
}

pub fn read_offers(orders: &Value) -> Vec<Offer> {
    // Create an array of Offers from the serde Value
    let mut offers_list = Vec::new();
    for offer in orders.as_array().unwrap() {
        let offer_struct = Offer {
            r#type: value_to_str(&offer["type"]),
            id: value_to_str(&offer["id"]),
            energy: value_to_f32(&offer["energy"]),
            energy_rate: value_to_f32(&offer["energy_rate"]),
            original_price: value_to_f32(&offer["original_price"]),
            requirements: Some(value_to_str(&offer["requirements"])),
            seller_origin: value_to_str(&offer["seller_origin"]),
            seller_origin_id: value_to_str(&offer["seller_origin_id"]),
            seller_id: value_to_str(&offer["seller_id"]),
            seller: value_to_str(&offer["seller"]),
            time_slot: value_to_datetime(&offer["time_slot"]),
            creation_time: value_to_datetime(&offer["creation_time"]),
        };
        offers_list.push(offer_struct);
    }
    offers_list
}

pub fn process_market_id_for_pay_as_bid(obj: &Value, market_id: &str) -> Vec<BidOfferMatch> {
    let mut matches = Vec::new();
    // Create a MatchingData Struct for the pay as bid algorithm
    for (_timestamp, obj) in obj.as_object().unwrap().iter() {
        let mut bids_list = Vec::new();
        let mut offers_list = Vec::new();
        for (key, orders) in obj.as_object().unwrap().iter() {
            if key == "bids" {
                bids_list = read_bids(orders);
            } else if key == "offers" {
                offers_list = read_offers(orders);
            } else {
                panic!("Unable to process market id: key not in ['bids', 'offers', 'market_id'].")
            }
        }
        let mut matching_data = MatchingData {
            bids: bids_list,
            offers: offers_list,
            market_id: market_id.to_string(),
        };
        let algorithm_result = matching_data.pay_as_bid();
        matches.extend(algorithm_result)
        // TODO - add tests for the result
    }
    matches
}

pub fn unwrap_offers_bids_response(payload: &str, client: &redis::Client) {
    // When a message from the bids_offers channel is received,
    // it extracts the market ids as keys to iterate over the
    // corresponding sets of bids and offers and trigger the
    // pay as bid algorithm.
    let value: Value = serde_json::from_str(&payload).unwrap();
    for (key, obj) in value.as_object().unwrap().iter() {
        if key == "bids_offers" {
            let mut matches = Vec::new();
            for (_market_id, obj) in obj.as_object().unwrap().iter() {
                matches.extend(process_market_id_for_pay_as_bid(obj, _market_id.as_str()));
            }

            client.get_connection().unwrap().publish::<String, String, redis::Value>(
                "external-matching-engine//recommendations/".to_string(),
                json!({"recommended_matches": matches}).to_string(),
            ).expect("Cannot publish Redis message to recommendations channel.");
        }
    };
}

pub fn unwrap_recommendations_response(payload: &str) {
    // When a message from the recommendations channel is received,
    // it is sent to the verifier function - TODO
    // Will be sent to TradeSettlement pallet, rejected matches are sent back by the OCW - TODO
    let _value: Value = serde_json::from_str(&payload).unwrap();
}

pub fn unwrap_tick_response(payload: &str, client: &redis::Client) {
    // When a message from the tick channel is received,
    // we check the slot completion %
    let value: Value = serde_json::from_str(&payload).unwrap();
    for (key, obj) in value.as_object().unwrap().iter() {
        if key == "slot_completion" {
            let slot_percent_str: &str = &obj.as_str().unwrap();
            let length = slot_percent_str.len();
            let slot_percent_int: i32 = slot_percent_str[..length - 1].parse().unwrap();
            // TODO: change this fast fix with the proper logic
            if slot_percent_int > 33 {
                client.get_connection().unwrap().publish::<String, String, redis::Value>(
                    "external-matching-engine//offers-bids/".to_string(), "{}".to_string()
                ).expect("Cannot publish Redis message to offers-bids channel.");
            }
        }
    }
}

pub async fn redis_subscribe(channels: Vec<String>, url: String) -> Result<(), Error> {

        let client = redis::Client::open(url)?;

        let mut con = client.get_connection()?;

        let mut pubsub = con.as_pubsub();
        for channel in channels {
            pubsub.psubscribe(channel)?;
        }

        loop {
            let msg = pubsub.get_message().unwrap();
            let payload: String = msg.get_payload().unwrap();
            let channel_name = msg.get_channel_name();
            match channel_name {
                "external-matching-engine//offers-bids/response/" => unwrap_offers_bids_response(&payload, &client),
                "external-matching-engine//recommendations/" => unwrap_recommendations_response(&payload),
                "external-matching-engine//events/" => unwrap_tick_response(&payload, &client),
                _ => unwrap_recommendations_response(&payload),
            };
        }
}