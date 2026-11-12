use gsy_analytics_engine::mapping::{
    meter_readings, onchain_id, trade_record, usable_point_ids, MarketIndex, ReadingStats,
};
use gsy_analytics_engine::model::{CommunityRef, MeterReading, TradeStatus};
use gsy_analytics_engine::period::Window;
use primitives::db_api_schema::profiles::{
    FlowDirection, MeasurementPointSchema, MeasurementPointType, TimeseriesSchema,
};
use primitives::db_api_schema::trades::{DbTradeSchema, TradeParameters};
use primitives::utils::{
    bytes16_to_hex, create_encrypted_bytes16_from_string, generate_market_id,
    timestamp_to_string_with_padding,
};
use primitives::MarketType;
use std::collections::HashMap;

const SLOT: i64 = 1_758_621_600; // 2025-09-23 10:00 UTC
const SLOT_LENGTH: i64 = 900;

fn window() -> Window {
    Window {
        start: SLOT,
        end: SLOT + 2 * SLOT_LENGTH,
        slot_length: SLOT_LENGTH,
    }
}

fn communities() -> Vec<CommunityRef> {
    ["Pilot1", "Pilot2"]
        .into_iter()
        .map(|id| CommunityRef {
            community_id: id.to_string(),
            community_name: format!("{} name", id),
        })
        .collect()
}

fn market_hex(community_id: &str, market_type: MarketType, slot: i64) -> String {
    bytes16_to_hex(generate_market_id(community_id, market_type, slot as u64))
}

fn db_trade(market_id: String) -> DbTradeSchema {
    DbTradeSchema {
        trade_uuid: "trade-1".to_string(),
        status: TradeStatus::Settled,
        seller: "0xseller".to_string(),
        buyer: "0xbuyer".to_string(),
        market_id,
        time_slot: (SLOT + SLOT_LENGTH) as u64,
        creation_time: SLOT as u64,
        offer_hash: "0xoffer".to_string(),
        bid_hash: "0xbid".to_string(),
        residual_offer_id: None,
        residual_bid_id: None,
        parameters: TradeParameters {
            selected_energy_kWh: 3.0,
            energy_rate: 0.12,
        },
    }
}

#[test]
fn onchain_id_matches_the_hash_used_for_orders() {
    assert_eq!(
        onchain_id("owner-1"),
        bytes16_to_hex(create_encrypted_bytes16_from_string("owner-1"))
    );
}

#[test]
fn market_index_maps_every_market_type_and_slot_to_its_community() {
    let index = MarketIndex::build(&communities(), &window());

    for market_type in [MarketType::Spot, MarketType::Flex, MarketType::Settlement] {
        for slot in [SLOT, SLOT + SLOT_LENGTH] {
            assert_eq!(
                index.community_for(&market_hex("Pilot1", market_type.clone(), slot)),
                Some("Pilot1")
            );
            assert_eq!(
                index.community_for(&market_hex("Pilot2", market_type.clone(), slot)),
                Some("Pilot2")
            );
        }
    }
    assert_eq!(
        index.community_for(&market_hex("Pilot1", MarketType::Spot, SLOT).to_uppercase()),
        Some("Pilot1")
    );
}

#[test]
fn markets_outside_the_window_or_of_unknown_communities_are_unassigned() {
    let index = MarketIndex::build(&communities(), &window());

    assert_eq!(
        index.community_for(&market_hex(
            "Pilot1",
            MarketType::Spot,
            SLOT + 2 * SLOT_LENGTH
        )),
        None
    );
    assert_eq!(
        index.community_for(&market_hex("Pilot3", MarketType::Spot, SLOT)),
        None
    );
    assert_eq!(index.community_for("0xnot-a-market"), None);
}

#[test]
fn trade_record_keeps_trade_fields_and_resolves_the_community() {
    let index = MarketIndex::build(&communities(), &window());
    let trade = db_trade(market_hex("Pilot2", MarketType::Flex, SLOT + SLOT_LENGTH));

    let record = trade_record(trade, &index);
    assert_eq!(record.trade_uuid, "trade-1");
    assert_eq!(record.community_id.as_deref(), Some("Pilot2"));
    assert_eq!(record.buyer, "0xbuyer");
    assert_eq!(record.time_slot, SLOT + SLOT_LENGTH);
    assert_eq!(record.energy_kwh, 3.0);
    assert_eq!(record.energy_rate, 0.12);
    assert_eq!(record.status, TradeStatus::Settled);

    let unassigned = trade_record(db_trade("0xforeign".to_string()), &index);
    assert_eq!(unassigned.community_id, None);
}

fn point(
    measurement_id: &str,
    asset_name: &str,
    community: Option<&str>,
) -> MeasurementPointSchema {
    MeasurementPointSchema {
        point_type: MeasurementPointType::Measurement,
        measurement_id: measurement_id.to_string(),
        property_measured: "energy_measured".to_string(),
        unit: "kWh".to_string(),
        direction: FlowDirection::Import,
        energy_accumulated: false,
        time_resolution: "PT15M".to_string(),
        phase: 0,
        asset_name: asset_name.to_string(),
        datasource_name: community.map(str::to_string),
    }
}

fn value(measurement_point: &str, timestamp: i64, energy_kwh: f64) -> TimeseriesSchema {
    TimeseriesSchema {
        measurement_point: measurement_point.to_string(),
        timestamp: timestamp_to_string_with_padding(timestamp as u64),
        value: energy_kwh,
    }
}

fn owners() -> HashMap<String, String> {
    HashMap::from([
        ("facility-a".to_string(), "owner-a".to_string()),
        ("facility-b".to_string(), "owner-b".to_string()),
    ])
}

#[test]
fn readings_are_keyed_by_the_owner_onchain_id_and_keep_their_sign() {
    let points = vec![
        point(
            "measurement:Pilot1:facility-a",
            "facility-a",
            Some("Pilot1"),
        ),
        point(
            "measurement:Pilot1:facility-b",
            "facility-b",
            Some("Pilot1"),
        ),
    ];
    let values = vec![
        value("measurement:Pilot1:facility-a", SLOT, 4.0),
        value("measurement:Pilot1:facility-b", SLOT, -5.0),
    ];

    let (readings, stats) = meter_readings(&points, values, &owners());
    assert_eq!(
        readings,
        vec![
            MeterReading {
                community_id: "Pilot1".to_string(),
                facility: onchain_id("owner-a"),
                time_slot: SLOT,
                energy_kwh: 4.0,
            },
            MeterReading {
                community_id: "Pilot1".to_string(),
                facility: onchain_id("owner-b"),
                time_slot: SLOT,
                energy_kwh: -5.0,
            },
        ]
    );
    assert_eq!(stats, ReadingStats::default());
}

#[test]
fn unusable_points_are_skipped_and_counted() {
    let mut forecast = point("forecast", "facility-a", Some("Pilot1"));
    forecast.point_type = MeasurementPointType::Forecast;
    let mut watt_hours = point("wh", "facility-a", Some("Pilot1"));
    watt_hours.unit = "Wh".to_string();
    let mut accumulated = point("accumulated", "facility-a", Some("Pilot1"));
    accumulated.energy_accumulated = true;
    let mut hourly = point("hourly", "facility-a", Some("Pilot1"));
    hourly.time_resolution = "PT1H".to_string();
    let no_community = point("no-community", "facility-a", None);
    let blank_community = point("blank-community", "facility-a", Some(" "));
    let mut upper_case_unit = point("upper", "facility-a", Some("Pilot1"));
    upper_case_unit.unit = "KWH".to_string();

    let points = vec![
        forecast,
        watt_hours,
        accumulated,
        hourly,
        no_community,
        blank_community,
        upper_case_unit,
    ];
    let values = points
        .iter()
        .map(|point| value(&point.measurement_id, SLOT, 1.0))
        .collect();

    assert_eq!(usable_point_ids(&points), vec!["upper".to_string()]);
    let (readings, stats) = meter_readings(&points, values, &owners());
    assert_eq!(readings.len(), 1);
    assert_eq!(stats.skipped_points, 6);
}

#[test]
fn values_without_owner_or_valid_timestamp_are_counted() {
    let points = vec![
        point("known", "facility-a", Some("Pilot1")),
        point("unknown-owner", "facility-z", Some("Pilot1")),
    ];
    let mut bad_timestamp = value("known", SLOT, 1.0);
    bad_timestamp.timestamp = "not-a-timestamp".to_string();
    let values = vec![
        value("known", SLOT, 1.0),
        value("unknown-owner", SLOT, 1.0),
        bad_timestamp,
        value("not-a-point", SLOT, 1.0),
    ];

    let (readings, stats) = meter_readings(&points, values, &owners());
    assert_eq!(readings.len(), 1);
    assert_eq!(
        stats,
        ReadingStats {
            skipped_points: 0,
            values_without_owner: 1,
            invalid_timestamps: 1,
        }
    );
}

#[test]
fn padded_timestamps_parse_back_to_unix_seconds() {
    let points = vec![point("known", "facility-a", Some("Pilot1"))];
    let values = vec![value("known", SLOT, 1.0)];
    assert_eq!(values[0].timestamp, "00000000001758621600");

    let (readings, _) = meter_readings(&points, values, &owners());
    assert_eq!(readings[0].time_slot, SLOT);
}
