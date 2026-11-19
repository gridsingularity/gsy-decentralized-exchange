import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("seed", Path(__file__).resolve().parents[1] / "seed_dummy_trades.py")
seed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seed)


class TradeSeedTests(unittest.TestCase):
    def dataset(self):
        start = seed.parse_date("2026-11-01")
        return seed.generate_dataset(start, start + seed.DAY_SECS, 3, 42, start)

    @contextlib.contextmanager
    def saved_dataset(self):
        trades = self.dataset()
        start = seed.parse_ts(trades[0]["timestamp"])
        meta = {
            "start": start,
            "end": start + seed.DAY_SECS,
            "per_slot": 3,
            "seed": 42,
            "as_of": start,
            "community_id": seed.COMMUNITY_ID,
            "market_type": seed.MARKET_TYPE,
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dataset.json"
            path.write_text(json.dumps({"meta": meta, "trades": trades}))
            yield SimpleNamespace(
                dataset=str(path),
                rest_url="http://unused-rest",
                gateway_url="http://unused-gateway",
                exhaustive=True,
                check_range_limit=False,
            ), meta, trades

    def test_dataset_uses_canonical_ids_and_preserves_other_seeded_values(self):
        trades = self.dataset()
        entity_keys = {"tradeId", "marketId", "bidId", "offerId", "residualBidId", "residualOfferId"}
        without_ids = lambda items: [{k: v for k, v in t.items() if k not in entity_keys} for t in items]
        self.assertEqual(without_ids(trades), without_ids(self.dataset()))
        self.assertEqual(len({t["tradeId"] for t in trades}), len(trades))
        self.assertEqual(seed.schema_errors({"requestId": "test", "success": True, "data": trades}), [])
        self.assertTrue({t["buyerId"] for t in trades} <= seed.ROSTER)
        markets = seed.market_ids_by_slot(trades)
        self.assertEqual(len(markets), seed.DAY_SECS // seed.SLOT_SECS)
        self.assertEqual(len(set(markets.values())), len(markets))
        for trade in trades:
            self.assertEqual(trade["marketId"], markets[seed.parse_ts(trade["timestamp"])])
            market_uuid = seed.uuid.UUID(trade["marketId"])
            self.assertEqual(market_uuid.version, 4)
            self.assertEqual(market_uuid.variant, seed.uuid.RFC_4122)
            self.assertEqual(trade["marketId"], str(market_uuid))
            for key in entity_keys:
                if trade.get(key) is not None:
                    identifier = seed.uuid.UUID(trade[key])
                    self.assertEqual(identifier.version, 4)
                    self.assertEqual(trade[key], str(identifier))

    def test_regeneration_assigns_fresh_market_ids(self):
        first = seed.market_ids_by_slot(self.dataset())
        second = seed.market_ids_by_slot(self.dataset())
        self.assertEqual(first.keys(), second.keys())
        self.assertTrue(set(first.values()).isdisjoint(second.values()))

    def test_dataset_rejects_conflicting_market_slot_references(self):
        trades = self.dataset()
        conflicting_slot = [{**trades[0], "marketId": trades[3]["marketId"]}, trades[1]]
        with self.assertRaisesRegex(ValueError, "multiple market IDs"):
            seed.market_ids_by_slot(conflicting_slot)
        reused_market = [trades[0], {**trades[3], "marketId": trades[0]["marketId"]}]
        with self.assertRaisesRegex(ValueError, "reuses a market ID"):
            seed.market_ids_by_slot(reused_market)

    def test_saved_dataset_and_stats_reuse_market_ids(self):
        with self.saved_dataset() as (args, meta, trades):
            output = io.StringIO()
            with patch.object(seed, "cmd_generate", side_effect=AssertionError("must reuse dataset")), \
                    patch.object(seed.uuid, "uuid4", side_effect=AssertionError("must reuse IDs")), \
                    contextlib.redirect_stdout(output):
                self.assertEqual(seed.ensure_dataset(args), (meta, trades))
                seed.dataset_stats(meta, trades)
            self.assertIn(f"market ref : slot {meta['start']} -> {trades[0]['marketId']}", output.getvalue())

    def test_preflight_queries_saved_market_id(self):
        with self.saved_dataset() as (args, _, trades):
            response = ({"requestId": "preflight", "success": True, "data": []}, 0.1, 10)
            with patch.object(seed, "ewds_trades_query", return_value=response) as query, \
                    patch.object(seed.uuid, "uuid4", side_effect=AssertionError("must reuse IDs")), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertTrue(seed.cmd_preflight(args))
            query.assert_called_once_with(args, market_id=trades[0]["marketId"])

    def test_validation_queries_saved_markets_and_an_absent_random_id(self):
        with self.saved_dataset() as (args, _, trades):
            market_ids = set(seed.market_ids_by_slot(trades).values())
            absent_uuid = seed.uuid.uuid4()

            def query_response(_args, market_id):
                return ({
                    "requestId": "validate",
                    "success": True,
                    "data": list(seed.expected_for_market(trades, market_id).values()),
                }, 0.1, 10)

            # Exercise the guard that keeps the negative-control ID outside the dataset.
            duplicate_uuid = seed.uuid.UUID(trades[0]["marketId"])
            with patch.object(seed, "rest_get_trades", side_effect=lambda a, m: query_response(a, m)[0]["data"]) as rest, \
                    patch.object(seed, "ewds_trades_query", side_effect=query_response) as ewds, \
                    patch.object(seed, "collection_counts", return_value={"trades": len(trades)}), \
                    patch.object(seed.uuid, "uuid4", side_effect=[duplicate_uuid, absent_uuid]), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertTrue(seed.cmd_validate(args))
            self.assertEqual({call.args[1] for call in rest.call_args_list}, market_ids)
            self.assertEqual({call.kwargs["market_id"] for call in ewds.call_args_list[:-1]}, market_ids)
            self.assertEqual(ewds.call_args.kwargs["market_id"], str(absent_uuid))
            self.assertNotIn(str(absent_uuid), market_ids)

    def test_cleanup_only_targets_dataset_ids_in_bounded_batches(self):
        trades = self.dataset() * 4
        filters = [json.loads(query) for query in seed.dataset_filters(trades)]
        self.assertTrue(all(len(query["trade_uuid"]["$in"]) <= 500 for query in filters))
        self.assertEqual([id for query in filters for id in query["trade_uuid"]["$in"]], [t["tradeId"] for t in trades])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dataset.json"
            path.write_text(json.dumps({"meta": {"community_id": seed.COMMUNITY_ID}, "trades": trades}))
            with patch.object(seed, "mongosh", return_value="1") as mongo, \
                    patch.object(seed.uuid, "uuid4", side_effect=AssertionError("must use saved IDs")), \
                    contextlib.redirect_stdout(io.StringIO()):
                seed.cmd_cleanup(SimpleNamespace(dataset=str(path)))
            self.assertEqual(mongo.call_count, len(filters))
            for call, query in zip(mongo.call_args_list, filters):
                self.assertIn(json.dumps(query), call.args[1])
                self.assertNotIn("$regex", call.args[1])

    def test_old_identifiers_require_regeneration(self):
        trade = self.dataset()[0]
        for key, value in [("tradeId", "dummy-trade-1"), ("buyerId", " Load1"), ("bidId", "0x" + "0" * 32)]:
            with self.subTest(key=key):
                self.assertTrue(seed.schema_errors({"requestId": "test", "success": True, "data": [{**trade, key: value}]}))

if __name__ == "__main__":
    unittest.main()
