"""Validate entity UUIDv4 and opaque actor IDs against the actual order and query schemas."""

import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

SCHEMAS = Path(__file__).resolve().parents[1] / "intelligent"
UUID = "ABABABAB-ABAB-4BAB-8BAB-ABABABABABAB"
HEX = UUID.replace("-", "")
VALID = [UUID, UUID.lower()]
NIL = ["0" * 32, "0x" + "0" * 32, "00000000-0000-0000-0000-000000000000"]
INVALID = [
    HEX, "0x" + HEX, UUID.replace("4BAB", "1BAB"), UUID.replace("8BAB", "7BAB"),
    "", "0x1234", "0X" + HEX, " " + UUID, UUID + "\n", UUID + " ",
    "{" + UUID + "}", "urn:uuid:" + UUID, "0x" + UUID,
    HEX + "----", "ABABABABABAB-ABAB-ABAB-ABAB-ABABABAB", "g" * 32,
]


def valid_actor_id(value):
    return bool(value) and value == value.strip()


def validator(filename):
    resources = Registry().with_resources(
        (path.as_uri(), Resource.from_contents(json.loads(path.read_text())))
        for path in SCHEMAS.glob("*.json")
    )
    schema = json.loads((SCHEMAS / filename).read_text())
    # Resolve the schema's relative local references without network requests.
    schema["$id"] = (SCHEMAS / filename).as_uri()
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, registry=resources, format_checker=FormatChecker())


def order(identifier):
    return {
        "orderId": identifier, "marketId": identifier, "orderType": "bid",
        "orderStatus": "submitted", "createdBy": "facility-a",
        "timeSlot": "2026-11-04T10:00:00Z", "creationTime": "2026-11-04T09:40:00Z",
        "quantity": 1, "priceLimit": 10, "preferredTradingPartner": None,
    }


class IdentifierSchemas(unittest.TestCase):
    def test_trade_import_ids_and_optional_residuals(self):
        check = validator("int.trade.schema.v1.json")
        required = ["tradeId", "marketId", "bidId", "offerId"]
        trade = {key: UUID for key in required}
        trade.update(buyerId="Owner-MiXeD/LIC01", sellerId="owner-b", tradeStatus="matched", tradeQuantity=1, tradePrice=10,
                     timestamp="2026-11-04T10:00:00Z")
        for key in required + ["residualBidId", "residualOfferId"]:
            for value in VALID + INVALID + NIL + [None]:
                with self.subTest(key=key, value=value):
                    optional = key.startswith("residual") and value is None
                    self.assertEqual(check.is_valid({**trade, key: value}), value in VALID or optional)

    def test_trade_parties_are_offchain_identities(self):
        check = validator("int.trade.schema.v1.json")
        trade = dict(tradeId=UUID, marketId=UUID, bidId=UUID, offerId=UUID,
                     buyerId="owner-a", sellerId="owner-b", tradeStatus="matched",
                     tradeQuantity=1, tradePrice=10, timestamp="2026-11-04T10:00:00Z")
        for field in ["buyerId", "sellerId"]:
            for value in VALID + INVALID + NIL + ["Owner-MiXeD/LIC01"]:
                self.assertEqual(check.is_valid({**trade, field: value}), valid_actor_id(value))

    def test_id_mapping_requests(self):
        check = validator("int.ids.query.request.v1.json")
        for key in ["offchainId", "offchain_id"]:
            for value in VALID + ["facility-a", "0X" + HEX, "", " ", " padded", "padded\n"]:
                with self.subTest(key=key, value=value):
                    self.assertEqual(check.is_valid({
                        "requestId": "mapping", "operation": "ids.query", "payload": {key: value},
                    }), bool(value) and value == value.strip())

    def test_profile_ingress_community_ids_and_lossless_responses(self):
        event = validator("int.measurements.event.v1.json")
        response = validator("int.measurements.query.response.v1.json")
        forecast = validator("int.forecast.schema.v1.json")
        for identifier in VALID + INVALID + NIL + ["DS-1-MQTT"]:
            measurement = {
                "facilityId": "facility-a", "communityUuid": identifier,
                "timeSlot": "2026-11-04T10:00:00Z", "creationTime": "2026-11-04T10:01:00Z",
                "energyKwh": -1,
            }
            with self.subTest(identifier=identifier):
                self.assertEqual(event.is_valid({
                    "eventId": "profile-event", "eventType": "measurements.event",
                    "occurredAt": "2026-11-04T10:01:00Z", "data": [measurement],
                }), valid_actor_id(identifier))
                self.assertEqual(forecast.is_valid({**measurement, "creationTime": 1, "confidence": 1}), valid_actor_id(identifier))
                response.validate({"requestId": "profile-query", "success": True, "data": [measurement]})

    def test_measurement_query_filters_and_aliases(self):
        query = validator("int.measurements.query.request.v1.json")
        for key in ["communityUuid", "community_uuid"]:
            for identifier in VALID + INVALID + NIL:
                with self.subTest(key=key, identifier=identifier):
                    self.assertEqual(query.is_valid({
                        "requestId": "query", "operation": "measurements.query", "payload": {key: identifier},
                    }), valid_actor_id(identifier))
        for key in ["facilityId", "facility_id", "areaUuid"]:
            query.validate({"requestId": "query", "operation": "measurements.query", "payload": {
                key: UUID, "communityUuid": "0x" + HEX,
                "start_time": "2026-11-04T10:00:00Z", "end_time": "2026-11-04T10:01:00Z",
            }})

    def test_offchain_identity_fields(self):
        fields = [
            ("int.order.schema.v1.json", "createdBy"),
            ("int.order.schema.v1.json", "preferredTradingPartner"),
            ("int.facility.schema.v1.json", "facility_id"),
            ("int.facility.schema.v1.json", "owner_id"),
            ("int.measurement.schema.v1.json", "facilityId"),
            ("int.forecast.schema.v1.json", "facilityId"),
        ]
        schemas = [validator(file).schema["properties"][field] for file, field in fields]
        schemas.append(validator("int.site.schema.v1.json").schema["properties"]["facilities"]["items"])
        for schema in schemas:
            check = Draft202012Validator(schema)
            for identity in VALID + ["facility-a", "Facility A", "0X" + HEX]:
                with self.subTest(identity=identity, schema=schema):
                    check.validate(identity)
            for identity in ["", " ", " padded", "padded ", "padded\n"]:
                self.assertFalse(check.is_valid(identity))

    def test_order_submission_lifecycle_and_rejection_forms(self):
        check = validator("int.order.event.v1.json")
        for identifier in VALID:
            for status, event_type in [("submitted", "order.event"), ("executed", "orderStatus.event"), ("rejected", "orderStatus.event")]:
                with self.subTest(identifier=identifier, status=status):
                    payload = order(identifier)
                    payload["orderStatus"] = status
                    payload["createdBy"] = "Owner-MiXeD/LIC01"
                    payload["preferredTradingPartner"] = "0x" + HEX
                    check.validate({
                        "eventId": "opaque-event-id", "eventType": event_type,
                        "occurredAt": "2026-11-04T10:01:00Z", "data": [payload],
                    })
        check = validator("int.order.schema.v1.json")
        check.validate(order(UUID))  # Null preferred partner remains allowed.
        for identifier in INVALID + ["0" * 32, "0x" + "0" * 32, "00000000-0000-0000-0000-000000000000"]:
            for field in ["orderId", "marketId"]:
                with self.subTest(identifier=identifier, field=field):
                    payload = order(UUID)
                    payload[field] = identifier
                    self.assertFalse(check.is_valid(payload))

    def test_query_market_identifiers(self):
        for operation in ["orders.query", "trades.query", "markets.query", "clearing_results.query"]:
            check = validator(f"int.{operation}.request.v1.json")
            for identifier in VALID + INVALID + NIL:
                with self.subTest(operation=operation, identifier=identifier):
                    payload = {"requestId": "opaque-request", "operation": operation, "payload": {"marketId": identifier}}
                    self.assertEqual(check.is_valid(payload), identifier in VALID)

    def test_community_event_and_query_identifiers(self):
        event = validator("int.community.event.v1.json")
        query = validator("int.markets.query.request.v1.json")
        for identifier in VALID + INVALID + NIL:
            with self.subTest(identifier=identifier):
                self.assertEqual(event.is_valid({
                    "eventId": "opaque-event-id", "eventType": "community.event",
                    "occurredAt": "2026-11-04T10:01:00Z",
                    "data": [{"communityId": identifier, "communityName": "Test", "sites": []}],
                }), valid_actor_id(identifier))
                self.assertEqual(query.is_valid({
                    "requestId": "opaque-request", "operation": "markets.query",
                    "payload": {"communityId": identifier},
                }), valid_actor_id(identifier))

    def test_market_output_preserves_offchain_community(self):
        check = validator("int.market.schema.v1.json")
        payload = {
            "marketId": "808f0603-acbe-4696-8d3d-a1cd0cb3f5f4",
            "communityId": "Community-LIC01", "marketType": "spot",
            "matchingAlgorithm": "pay_as_bid",
            **{field: "2026-11-04T10:00:00Z" for field in [
                "openingTime", "closingTime", "deliveryStartTime", "deliveryEndTime", "createdAt",
            ]},
        }
        check.validate(payload)
        self.assertFalse(check.is_valid({**payload, "marketId": NIL[2]}))
        for community in ["Community-LIC01", UUID, "0x" + HEX, NIL[2]]:
            check.validate({**payload, "communityId": community})
        for community in ["", " ", " padded", "padded\n"]:
            self.assertFalse(check.is_valid({**payload, "communityId": community}))

    def test_clearing_result_market_identifier(self):
        schema = validator("int.clearing-result.schema.v1.json").schema["properties"]["marketId"]
        check = Draft202012Validator(schema)
        for identifier in VALID + INVALID + NIL:
            with self.subTest(identifier=identifier):
                self.assertEqual(check.is_valid(identifier), identifier in VALID)

    def test_market_time_series_entity_ids(self):
        check = validator("int.market-time-series.schema.v1.json")
        payload = {
            "communityId": "Community-LIC01",
            "periodFrom": "2026-11-04T10:00:00Z",
            "periodUntil": "2026-11-04T11:00:00Z",
            "granularity": "15min",
        }
        for identifier in VALID + INVALID + NIL:
            with self.subTest(identifier=identifier):
                self.assertEqual(
                    check.is_valid({**payload, "marketIds": [identifier]}), identifier in VALID
                )


if __name__ == "__main__":
    unittest.main()
