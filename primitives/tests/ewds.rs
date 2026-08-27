use primitives::db_api_schema::{
    grid_topology::EnergyCommunitySchema,
    market::{MarketSchema, MarketType, MatchingAlgorithm},
    orders::{DbOrderSchema, DbRequirements, EnergyType, OrderEnum, OrderStatus},
    trades::{
        ClearingResultSchema, ClearingStatus, DbTradeSchema, NoBidReason, TradeParameters,
        TradeStatus,
    },
};
use primitives::ewds::EwdsOperation;
use primitives::ewds::dto::{
    EwdsClearingResultDto, EwdsCommunityDto, EwdsMarketDto, EwdsOrderDto, EwdsTradeDto,
    energy_type_from_ewds, energy_type_to_ewds,
};
use serde_json::Value;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_conversion_round_trips_through_ewds_dto() {
        let expected = EnergyCommunitySchema {
            community_id: "a5657b6e-b0b2-46ee-87d6-1e29470339a7".to_string(),
            community_name: "Test community".to_string(),
            sites: vec!["site-id".to_string()],
        };

        let dto = EwdsCommunityDto::from(expected.clone());
        let serialized = serde_json::to_value(&dto).unwrap();

        assert_eq!(
            serialized.get("communityId").and_then(Value::as_str),
            Some(expected.community_id.as_str())
        );
        assert_eq!(EnergyCommunitySchema::from(dto), expected);
    }

    fn order() -> DbOrderSchema {
        DbOrderSchema {
            order_id: "order-id".to_string(),
            status: OrderStatus::Open,
            order_type: OrderEnum::Bid,
            area_uuid: "actor-id".to_string(),
            market_id: "market-id".to_string(),
            time_slot: 10,
            creation_time: 9,
            energy_kWh: 4.5,
            energy_rate: 12.0,
            created_by: "actor-id".to_string(),
            requirements: Some(DbRequirements {
                trading_partner_id: Some("partner-id".to_string()),
                energy_type: Some(EnergyType::Green),
                preferred_energy_rate: Some(12.0),
            }),
            attributes: None,
        }
    }

    #[test]
    fn db_to_ewds_maps_fields() {
        let dto = EwdsOrderDto::from(order());

        assert_eq!(dto.order_id, "order-id");
        assert_eq!(dto.market_id, "market-id");
        assert_eq!(dto.order_type, "bid");
        assert_eq!(dto.order_status, "submitted");
        assert_eq!(dto.time_slot, 10);
        assert_eq!(dto.quantity, 4.5);
        assert_eq!(dto.price_limit, 12.0);
        assert_eq!(dto.energy_source_preference.as_deref(), Some("GREEN"));
        assert_eq!(dto.energy_type.as_deref(), Some("NONE")); // no attributes -> default
        assert_eq!(dto.preferred_trading_partner.as_deref(), Some("partner-id"));
        assert_eq!(dto.created_by, "actor-id");
    }

    #[test]
    fn ewds_to_db_maps_fields() {
        let db = DbOrderSchema::try_from(EwdsOrderDto::from(order()))
            .expect("EWDS order should convert to DB schema");

        assert_eq!(db.order_id, "order-id");
        assert_eq!(db.market_id, "market-id");
        assert_eq!(db.order_type, OrderEnum::Bid);
        assert_eq!(db.status, OrderStatus::Open);
        assert_eq!(db.time_slot, 10);
        assert_eq!(db.energy_kWh, 4.5);
        assert_eq!(db.energy_rate, 12.0);
        assert_eq!(db.created_by, "actor-id");

        // requirements rebuilt from energy_source_preference + preferred_trading_partner
        let req = db.requirements.expect("requirements present");
        assert_eq!(req.trading_partner_id.as_deref(), Some("partner-id"));
        assert_eq!(req.energy_type, Some(EnergyType::Green));
        assert_eq!(req.preferred_energy_rate, Some(12.0));

        // attributes rebuilt from energy_type ("NONE"); TryFrom always sets
        // trading_partner_id: None on attributes, and source energy_type is None.
        let attr = db.attributes.expect("attributes present");
        assert_eq!(attr.trading_partner_id.as_deref(), None);
        assert_eq!(attr.energy_type, EnergyType::None);
    }

    #[test]
    fn energy_type_round_trips() {
        for et in [
            EnergyType::Green,
            EnergyType::Pv,
            EnergyType::Hydro,
            EnergyType::Biomass,
            EnergyType::Battery,
            EnergyType::None,
        ] {
            let s = energy_type_to_ewds(&et);
            assert_eq!(energy_type_from_ewds(s).unwrap(), et);
        }
    }

    #[test]
    fn unknown_energy_type_is_error() {
        assert!(energy_type_from_ewds("PLUTONIUM").is_err());
    }

    fn trade() -> DbTradeSchema {
        DbTradeSchema {
            trade_uuid: "trade-id".to_string(),
            status: TradeStatus::Settled,
            seller: "seller-id".to_string(),
            buyer: "buyer-id".to_string(),
            market_id: "market-id".to_string(),
            time_slot: 10,
            creation_time: 10, // equal to time_slot so round-trip holds
            offer_hash: "offer-hash".to_string(),
            bid_hash: "bid-hash".to_string(),
            residual_offer_id: Some("res-offer".to_string()),
            residual_bid_id: Some("res-bid".to_string()),
            parameters: TradeParameters {
                selected_energy_kWh: 4.5,
                energy_rate: 12.0,
            },
        }
    }

    #[test]
    fn db_to_ewds_trade_maps_fields() {
        let dto = EwdsTradeDto::from(trade());

        assert_eq!(dto.trade_id, "trade-id");
        assert_eq!(dto.market_id, "market-id");
        assert_eq!(dto.bid_id, "bid-hash");
        assert_eq!(dto.buyer_id, "buyer-id");
        assert_eq!(dto.residual_bid_id.as_deref(), Some("res-bid"));
        assert_eq!(dto.offer_id, "offer-hash");
        assert_eq!(dto.seller_id, "seller-id");
        assert_eq!(dto.residual_offer_id.as_deref(), Some("res-offer"));
        assert_eq!(dto.trade_status, "settled");
        assert_eq!(dto.trade_quantity, 4.5);
        assert_eq!(dto.trade_price, 12.0);
        assert_eq!(dto.timestamp, 10);
    }

    #[test]
    fn ewds_to_db_trade_maps_fields() {
        let db = DbTradeSchema::try_from(EwdsTradeDto::from(trade()))
            .expect("EWDS trade should convert to DB schema");

        assert_eq!(db.trade_uuid, "trade-id");
        assert_eq!(db.status, TradeStatus::Settled);
        assert_eq!(db.seller, "seller-id");
        assert_eq!(db.buyer, "buyer-id");
        assert_eq!(db.market_id, "market-id");
        assert_eq!(db.time_slot, 10);
        assert_eq!(db.creation_time, 10);
        assert_eq!(db.offer_hash, "offer-hash");
        assert_eq!(db.bid_hash, "bid-hash");
        assert_eq!(db.residual_offer_id.as_deref(), Some("res-offer"));
        assert_eq!(db.residual_bid_id.as_deref(), Some("res-bid"));
        assert_eq!(db.parameters.selected_energy_kWh, 4.5);
        assert_eq!(db.parameters.energy_rate, 12.0);
    }

    #[test]
    fn trade_round_trips_when_creation_time_equals_time_slot() {
        let expected = trade();
        let actual = DbTradeSchema::try_from(EwdsTradeDto::from(expected.clone()))
            .expect("round trip should succeed");
        assert_eq!(actual, expected);
    }

    #[test]
    fn creation_time_is_lost_when_it_differs_from_time_slot() {
        let mut original = trade();
        original.creation_time = 99; // differs from time_slot (10)

        let actual = DbTradeSchema::try_from(EwdsTradeDto::from(original.clone())).unwrap();

        assert_ne!(actual, original);
        assert_eq!(actual.creation_time, original.time_slot); // both come from timestamp
    }

    // ---- ClearingResultDto tests ----

    fn clearing_result() -> ClearingResultSchema {
        ClearingResultSchema {
            market_id: "market-id".to_string(),
            clearing_status: ClearingStatus::Final,
            no_bid_reason: None,
            clearing_price: 15.0,
            total_supply: 100.0,
            total_demand: 80.0,
            traded_quantity: 75.0,
            num_trades: 3,
            tx_hash: "0xabc".to_string(),
            clearing_time: 42,
        }
    }

    #[test]
    fn db_to_ewds_clearing_maps_fields() {
        let dto = EwdsClearingResultDto::from(clearing_result());

        assert_eq!(dto.market_id, "market-id");
        assert_eq!(dto.clearing_status, "final");
        assert_eq!(dto.no_bid_reason, None);
        assert_eq!(dto.clearing_price, 15.0);
        assert_eq!(dto.total_supply, 100.0);
        assert_eq!(dto.total_demand, 80.0);
        assert_eq!(dto.trade_quantity, 75.0); // traded_quantity -> trade_quantity
        assert_eq!(dto.num_trades, 3);
        assert_eq!(dto.tx_hash, "0xabc");
        assert_eq!(dto.created_at, 42); // clearing_time -> created_at
    }

    #[test]
    fn ewds_to_db_clearing_maps_fields() {
        let db = ClearingResultSchema::try_from(EwdsClearingResultDto::from(clearing_result()))
            .expect("EWDS clearing result should convert to DB schema");

        assert_eq!(db.market_id, "market-id");
        assert_eq!(db.clearing_status, ClearingStatus::Final);
        assert_eq!(db.no_bid_reason, None);
        assert_eq!(db.clearing_price, 15.0);
        assert_eq!(db.total_supply, 100.0);
        assert_eq!(db.total_demand, 80.0);
        assert_eq!(db.traded_quantity, 75.0);
        assert_eq!(db.num_trades, 3);
        assert_eq!(db.tx_hash, "0xabc");
        assert_eq!(db.clearing_time, 42);
    }

    #[test]
    fn clearing_result_round_trips() {
        let expected = clearing_result();
        let actual = ClearingResultSchema::try_from(EwdsClearingResultDto::from(expected.clone()))
            .expect("round trip should succeed");
        assert_eq!(actual, expected);
    }

    #[test]
    fn clearing_result_no_bid_round_trips() {
        let mut expected = clearing_result();
        expected.clearing_status = ClearingStatus::NoBid;
        expected.no_bid_reason = Some(NoBidReason::HardConstraints);

        let dto = EwdsClearingResultDto::from(expected.clone());
        assert_eq!(dto.clearing_status, "no_bid");
        assert_eq!(dto.no_bid_reason.as_deref(), Some("hard_constraints"));

        let actual = ClearingResultSchema::try_from(dto).expect("round trip should succeed");
        assert_eq!(actual, expected);
    }

    #[test]
    fn clearing_status_round_trips() {
        for status in [
            ClearingStatus::Final,
            ClearingStatus::Partial,
            ClearingStatus::Rejected,
            ClearingStatus::NoBid,
        ] {
            let dto = EwdsClearingResultDto::from(ClearingResultSchema {
                clearing_status: status.clone(),
                ..clearing_result()
            });
            let parsed = ClearingStatus::from_str(&dto.clearing_status).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn no_bid_reason_round_trips() {
        for reason in [
            NoBidReason::InvalidInputs,
            NoBidReason::StaleInput,
            NoBidReason::HardConstraints,
            NoBidReason::PolicyUnavailable,
            NoBidReason::DeadlineMissed,
            NoBidReason::Timeout,
            NoBidReason::OperatorDisabled,
            NoBidReason::MarketReject,
        ] {
            let mut src = clearing_result();
            src.clearing_status = ClearingStatus::NoBid;
            src.no_bid_reason = Some(reason.clone());

            let dto = EwdsClearingResultDto::from(src);
            let back = ClearingResultSchema::try_from(dto).unwrap();
            assert_eq!(back.no_bid_reason, Some(reason));
        }
    }

    #[test]
    fn unknown_clearing_status_is_error() {
        assert!(ClearingStatus::from_str("liquidated").is_err());
    }

    #[test]
    fn unknown_no_bid_reason_is_error() {
        assert!(NoBidReason::from_str("gremlins").is_err());
    }

    fn market() -> MarketSchema {
        MarketSchema {
            market_id: "market-id".to_string(),
            community_id: "community-id".to_string(),
            opening_time: "00000000001700000000".to_string(),
            closing_time: "00000000001700001800".to_string(),
            delivery_start_time: "00000000001700001800".to_string(),
            delivery_end_time: "00000000001700002700".to_string(),
            market_type: MarketType::Spot,
            matching_algorithm: MatchingAlgorithm::PayAsBid,
            created_at: "00000000001699999000".to_string(),
        }
    }

    #[test]
    fn market_db_to_ewds_maps_fields() {
        let dto = EwdsMarketDto::from(market());

        assert_eq!(dto.market_id, "market-id");
        assert_eq!(dto.community_id, "community-id");
        assert_eq!(dto.opening_time, "00000000001700000000");
        assert_eq!(dto.closing_time, "00000000001700001800");
        assert_eq!(dto.delivery_start_time, "00000000001700001800");
        assert_eq!(dto.delivery_end_time, "00000000001700002700");
        assert_eq!(dto.market_type, MarketType::Spot);
        assert_eq!(dto.matching_algorithm, MatchingAlgorithm::PayAsBid);
        assert_eq!(dto.created_at, "00000000001699999000");
    }

    #[test]
    fn market_round_trip_preserves_schema() {
        let src = market();
        let back = MarketSchema::from(EwdsMarketDto::from(src.clone()));
        assert_eq!(back, src);
    }

    #[test]
    fn market_dto_serialises_camel_case_with_wire_enums() {
        let json = serde_json::to_value(EwdsMarketDto::from(market())).unwrap();

        assert_eq!(json["marketId"], serde_json::json!("market-id"));
        assert_eq!(json["communityId"], serde_json::json!("community-id"));
        assert_eq!(
            json["openingTime"],
            serde_json::json!("00000000001700000000")
        );
        assert_eq!(
            json["deliveryStartTime"],
            serde_json::json!("00000000001700001800")
        );
        assert_eq!(json["marketType"], serde_json::json!("spot"));
        assert_eq!(json["matchingAlgorithm"], serde_json::json!("pay_as_bid"));
        assert_eq!(json["createdAt"], serde_json::json!("00000000001699999000"));
    }

    #[test]
    fn markets_query_operation_wire_name() {
        assert_eq!(EwdsOperation::MarketsQuery.as_str(), "markets.query");
        assert_eq!(
            serde_json::to_value(EwdsOperation::MarketsQuery).unwrap(),
            serde_json::json!("markets.query")
        );
        assert!(EwdsOperation::ALL.contains(&EwdsOperation::MarketsQuery));
    }
}
