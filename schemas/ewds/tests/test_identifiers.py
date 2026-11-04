"""Validate UUID/hex forms against the actual order and query schemas."""

import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

SCHEMAS = Path(__file__).resolve().parents[1] / "intelligent"
UUID = "ABABABAB-ABAB-ABAB-ABAB-ABABABABABAB"
HEX = UUID.replace("-", "")
VALID = [UUID, UUID.lower(), HEX, "0x" + HEX, "0x" + HEX.lower()]
NIL = ["0" * 32, "0x" + "0" * 32, "00000000-0000-0000-0000-000000000000"]
INVALID = [
    "", "0x1234", "0X" + HEX, " " + UUID, UUID + "\n", UUID + " ",
    "{" + UUID + "}", "urn:uuid:" + UUID, "0x" + UUID,
    HEX + "----", "ABABABABABAB-ABAB-ABAB-ABAB-ABABABAB", "g" * 32,
]


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
    def test_order_submission_lifecycle_and_rejection_forms(self):
        check = validator("int.order.event.v1.json")
        for identifier in VALID:
            for status, event_type in [("submitted", "order.event"), ("executed", "orderStatus.event"), ("rejected", "orderStatus.event")]:
                with self.subTest(identifier=identifier, status=status):
                    payload = order(identifier)
                    payload["orderStatus"] = status
                    payload["createdBy"] = "0x" + HEX.lower() if status == "executed" else "facility-a"
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
        for operation in ["orders.query", "trades.query", "markets.query"]:
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
                }), identifier in VALID)
                self.assertEqual(query.is_valid({
                    "requestId": "opaque-request", "operation": "markets.query",
                    "payload": {"communityId": identifier},
                }), identifier in VALID)

    def test_market_output_accepts_hash_uuids_and_rejects_nil(self):
        check = validator("int.market.schema.v1.json")
        payload = {
            "marketId": "808f0603-acbe-6696-4d3d-a1cd0cb3f5f4",
            "communityId": UUID.lower(), "marketType": "spot",
            "matchingAlgorithm": "pay_as_bid",
            **{field: "2026-11-04T10:00:00Z" for field in [
                "openingTime", "closingTime", "deliveryStartTime", "deliveryEndTime", "createdAt",
            ]},
        }
        check.validate(payload)
        for field in ["marketId", "communityId"]:
            self.assertFalse(check.is_valid({**payload, field: NIL[2]}))

    def test_clearing_result_market_identifier(self):
        schema = validator("int.clearing-result.schema.v1.json").schema["properties"]["marketId"]
        check = Draft202012Validator(schema)
        for identifier in VALID + INVALID + NIL:
            with self.subTest(identifier=identifier):
                self.assertEqual(check.is_valid(identifier), identifier in VALID)


if __name__ == "__main__":
    unittest.main()
