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

    def test_dataset_is_deterministic_and_uses_canonical_ids(self):
        trades = self.dataset()
        self.assertEqual(trades, self.dataset())
        self.assertEqual(len({t["tradeId"] for t in trades}), len(trades))
        self.assertEqual(seed.schema_errors({"requestId": "test", "success": True, "data": trades}), [])
        self.assertTrue({t["buyerId"] for t in trades} <= {seed.bytes16_id(name) for name in seed.ROSTER})
        self.assertEqual(seed.bytes16_id("facility-a"), "0x" + seed.hashlib.blake2b(b"facility-a", digest_size=16).hexdigest())
        self.assertEqual({t["marketId"] for t in trades}, {seed.market_id_for_slot(seed.parse_ts(t["timestamp"])) for t in trades})

    def test_cleanup_only_targets_dataset_ids_in_bounded_batches(self):
        trades = self.dataset() * 4
        filters = [json.loads(query) for query in seed.dataset_filters(trades)]
        self.assertTrue(all(len(query["trade_uuid"]["$in"]) <= 500 for query in filters))
        self.assertEqual([id for query in filters for id in query["trade_uuid"]["$in"]], [t["tradeId"] for t in trades])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dataset.json"
            path.write_text(json.dumps({"meta": {"community_id": seed.COMMUNITY_ID}, "trades": trades}))
            with patch.object(seed, "mongosh", return_value="1") as mongo, contextlib.redirect_stdout(io.StringIO()):
                seed.cmd_cleanup(SimpleNamespace(dataset=str(path)))
            self.assertEqual(mongo.call_count, len(filters))
            for call, query in zip(mongo.call_args_list, filters):
                self.assertIn(json.dumps(query), call.args[1])
                self.assertNotIn("$regex", call.args[1])

    def test_old_identifiers_require_regeneration(self):
        trade = self.dataset()[0]
        for key, value in [("tradeId", "dummy-trade-1"), ("buyerId", "Load1"), ("bidId", "0x" + "0" * 32)]:
            with self.subTest(key=key):
                self.assertTrue(seed.schema_errors({"requestId": "test", "success": True, "data": [{**trade, key: value}]}))


if __name__ == "__main__":
    unittest.main()
