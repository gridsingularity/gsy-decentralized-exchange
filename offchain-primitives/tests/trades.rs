use gsy_offchain_primitives::db_api_schema::trades::TradeSchema;

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents written before `status_updated_at` existed have no such key, and must still
    /// load. serde reads a missing `Option` field as `None` on its own, so this holds with or
    /// without the `#[serde(default)]` on the field; what it pins is that the field stays an
    /// `Option`, which is what actually carries the compatibility.
    #[test]
    fn trade_schema_deserializes_without_status_updated_at() {
        let json = serde_json::json!({
            "_id": "trade-1",
            "status": "Settled",
            "seller": "seller_account",
            "buyer": "buyer_account",
            "market_id": "market",
            "time_slot": 100,
            "trade_uuid": "0xtrade",
            "creation_time": 1677453190u64,
            "offer": {
                "seller": "seller_account",
                "nonce": 1,
                "offer_component": {
                    "area_uuid": "area",
                    "market_id": "market",
                    "time_slot": 100,
                    "creation_time": 1677453190u64,
                    "energy": 100.0,
                    "energy_rate": 10.0
                }
            },
            "offer_hash": "0xoffer",
            "bid": {
                "buyer": "buyer_account",
                "nonce": 1,
                "bid_component": {
                    "area_uuid": "area",
                    "market_id": "market",
                    "time_slot": 100,
                    "creation_time": 1677453190u64,
                    "energy": 100.0,
                    "energy_rate": 10.0
                }
            },
            "bid_hash": "0xbid",
            "residual_offer": null,
            "residual_bid": null,
            "parameters": {
                "selected_energy": 100.0,
                "energy_rate": 10.0,
                "trade_uuid": "0xtrade"
            }
        });

        let trade: TradeSchema =
            serde_json::from_value(json).expect("a trade without status_updated_at must still deserialize");
        assert_eq!(trade.status_updated_at, None);
    }
}
