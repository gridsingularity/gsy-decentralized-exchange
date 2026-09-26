use codec::{Decode, Encode};
use gsy_offchain_primitives::db_api_schema::profiles::{
    MeasurementCompleteness, MeasurementSchema, MeteringPointMeasurement,
};
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    fn measurement(
        energy_kwh: f64,
        metering_point: Option<MeteringPointMeasurement>,
    ) -> MeasurementSchema {
        MeasurementSchema {
            area_uuid: "area".to_string(),
            area_hash: "hash".to_string(),
            community_uuid: "community-1".to_string(),
            time_slot: 1_700_000_000,
            creation_time: 1_700_000_100,
            energy_kwh,
            metering_point,
        }
    }

    /// A per-area measurement as stored and posted before `metering_point` existed.
    fn per_area_json() -> serde_json::Value {
        json!({
            "area_uuid": "area",
            "area_hash": "hash",
            "community_uuid": "community-1",
            "time_slot": 1_700_000_000u64,
            "creation_time": 1_700_000_100u64,
            "energy_kwh": 1.5
        })
    }

    /// A building row whose second meter did not report for the slot.
    fn incomplete_building_row() -> MeasurementSchema {
        measurement(
            0.0,
            Some(MeteringPointMeasurement {
                name: "AICHouse11".to_string(),
                member_area_hashes: vec!["0xaaa".to_string(), "0xbbb".to_string()],
                completeness: MeasurementCompleteness::Incomplete,
                missing_meters: vec!["AIC35".to_string()],
            }),
        )
    }

    #[test]
    fn measurement_without_metering_point_key_deserializes_as_none() {
        let decoded: MeasurementSchema = serde_json::from_value(per_area_json()).unwrap();
        assert_eq!(decoded.metering_point, None);
        assert_eq!(decoded, measurement(1.5, None));

        let mut explicit_null = per_area_json();
        explicit_null["metering_point"] = serde_json::Value::Null;
        let decoded: MeasurementSchema = serde_json::from_value(explicit_null).unwrap();
        assert_eq!(decoded.metering_point, None);
    }

    #[test]
    fn measurement_without_metering_point_serializes_without_the_key() {
        let json = serde_json::to_value(measurement(1.5, None)).unwrap();
        assert!(json.get("metering_point").is_none());
        assert_eq!(json, per_area_json());
    }

    #[test]
    fn measurement_with_metering_point_round_trips_through_json() {
        let row = incomplete_building_row();
        let json = serde_json::to_value(&row).unwrap();
        assert_eq!(
            json["metering_point"],
            json!({
                "name": "AICHouse11",
                "member_area_hashes": ["0xaaa", "0xbbb"],
                "completeness": "incomplete",
                "missing_meters": ["AIC35"]
            })
        );
        let decoded: MeasurementSchema = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, row);
    }

    #[test]
    fn completeness_serializes_as_snake_case() {
        for (completeness, text) in [
            (MeasurementCompleteness::Complete, "complete"),
            (MeasurementCompleteness::Incomplete, "incomplete"),
            (MeasurementCompleteness::Missing, "missing"),
        ] {
            assert_eq!(serde_json::to_value(&completeness).unwrap(), json!(text));
            let decoded: MeasurementCompleteness = serde_json::from_value(json!(text)).unwrap();
            assert_eq!(decoded, completeness);
        }
    }

    #[test]
    fn measurement_round_trips_through_scale() {
        for row in [measurement(1.5, None), incomplete_building_row()] {
            let encoded = row.encode();
            let mut input = &encoded[..];
            assert_eq!(MeasurementSchema::decode(&mut input).unwrap(), row);
            assert!(input.is_empty());
        }
    }
}
