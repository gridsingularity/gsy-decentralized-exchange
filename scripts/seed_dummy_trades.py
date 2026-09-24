#!/usr/bin/env python3
"""Seed and validate dummy Trade data on the INTELLIGENT staging offchain-storage.

Plan: docs/plans/intelligent-staging-dummy-trades.md

Only the `trades` collection is written (via REST `POST /trades`); cleanup uses
mongosh inside the Mongo container over SSH. Standard library only.

Subcommands:
  generate   build the deterministic dataset and write it to --dataset (no network)
  preflight  EWDS trades.query round-trip on a 1-day window, expecting no dummy trades
  seed       POST the dataset in batches (aborts if dummy trades already exist)
  validate   REST full coverage + EWDS tradesQuery/tradesQueryResponse + isolation checks
  cleanup    delete every trade whose trade_uuid starts with the dummy prefix
  counts     print document counts of every collection

Unless --rest-url/--gateway-url are given, an SSH tunnel to --ssh-host is opened
for the duration of the command.
"""

import argparse
import hashlib
import json
import math
import random
import shlex
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from collections import Counter, defaultdict
from datetime import datetime, timezone

SLOT_SECS = 900
DAY_SECS = 86_400
TRADE_PREFIX = "dummy-trade-"
COMMUNITY_ID = "intelligent-dummy-community"
MARKET_TYPE = "spot"

PV_SELLERS = ["PV1", "PV2"]
BATTERY = "Battery1"
LOADS = ["Load1", "Load2", "Load3"]
ROSTER = set(PV_SELLERS + [BATTERY] + LOADS)
DAYTIME_HOURS = range(6, 18)  # UTC hours in which PV sells

TRADE_STATUSES = {"matched", "executed", "settled", "rejected"}
TRADE_KEYS = {
    "tradeId": str,
    "marketId": str,
    "bidId": str,
    "buyerId": str,
    "residualBidId": (str, type(None)),
    "offerId": str,
    "sellerId": str,
    "residualOfferId": (str, type(None)),
    "tradeStatus": str,
    "tradeQuantity": (int, float),
    "tradePrice": (int, float),
    "timestamp": int,
}
TRADE_REQUIRED = set(TRADE_KEYS) - {"residualBidId", "residualOfferId"}

MONGO_CONTAINER = "gsy-decentralized-exchange-mongodb-1"
MONGO_DB = "offchain_storage"

EWDS_TOPIC_OWNER = "integration.apps.intelligent.auth.ewc"
EWDS_TOPIC_VERSION = "1.0.0"
EWDS_REQUEST_FQCN = "gsy.intelligent.requests.pub"
EWDS_RESPONSE_FQCN = "gsy.intelligent.responses.sub"
EWDS_REQUEST_TOPIC = "tradesQuery"
EWDS_RESPONSE_TOPIC = "tradesQueryResponse"
EWDS_CLIENT_ID = "gsyseedvalidator" + EWDS_RESPONSE_TOPIC


# --------------------------------------------------------------------------
# Dataset
# --------------------------------------------------------------------------


def parse_date(value):
    return int(datetime.strptime(value, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp())


def fmt_ts(ts):
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%dT%H:%MZ")


def market_id_for_slot(slot):
    """Mirrors bytes16_to_hex(generate_market_id(community, MarketType::Spot, slot))."""
    digest = hashlib.blake2b(
        COMMUNITY_ID.encode() + MARKET_TYPE.encode() + slot.to_bytes(8, "big"),
        digest_size=16,
    ).hexdigest()
    return "0x" + digest


def pick_status(rng, slot, as_of):
    if slot + SLOT_SECS > as_of:
        return "matched"
    r = rng.random()
    if r < 0.90:
        return "settled"
    if r < 0.97:
        return "executed"
    return "rejected"


def pv_quantity(rng, slot):
    hour = (slot % DAY_SECS) / 3600
    bell = math.sin(math.pi * (hour - 6) / 12)  # 0 at 06:00/18:00, 1 at noon
    return max(0.1, min(5.0, 0.3 + 4.5 * bell * rng.uniform(0.7, 1.0)))


def trade_price(rng, slot):
    hour = (slot % DAY_SECS) / 3600
    price = 0.12 + rng.uniform(-0.03, 0.03)
    if 17 <= hour < 21:
        price += 0.10
    elif 11 <= hour < 15:
        price -= 0.02
    return max(0.08, min(0.30, price))


def generate_dataset(start, end, per_slot, seed, as_of):
    rng = random.Random(seed)
    trades = []
    counter = 0
    for slot in range(start, end, SLOT_SECS):
        hour = (slot % DAY_SECS) // 3600
        daytime = hour in DAYTIME_HOURS
        market_id = market_id_for_slot(slot)
        buyers_pool = LOADS + [BATTERY] if daytime else LOADS
        buyers = rng.sample(buyers_pool, per_slot)
        for buyer in buyers:
            counter += 1
            if daytime:
                seller = rng.choice(PV_SELLERS)
                quantity = pv_quantity(rng, slot)
            else:
                seller = BATTERY
                quantity = rng.uniform(0.1, 2.0)
            assert buyer != seller
            num = f"{counter:06d}"
            residual_bid = residual_offer = None
            if rng.random() < 0.10:
                if rng.random() < 0.5:
                    residual_bid = f"dummy-bid-{num}-r"
                else:
                    residual_offer = f"dummy-offer-{num}-r"
            trades.append(
                {
                    "tradeId": f"{TRADE_PREFIX}{num}",
                    "marketId": market_id,
                    "bidId": f"dummy-bid-{num}",
                    "buyerId": buyer,
                    "residualBidId": residual_bid,
                    "offerId": f"dummy-offer-{num}",
                    "sellerId": seller,
                    "residualOfferId": residual_offer,
                    "tradeStatus": pick_status(rng, slot, as_of),
                    "tradeQuantity": round(quantity, 4),
                    "tradePrice": round(trade_price(rng, slot), 4),
                    "timestamp": slot,
                }
            )
    return trades


def dataset_stats(meta, trades):
    print(f"range      : {fmt_ts(meta['start'])} .. {fmt_ts(meta['end'])} (end exclusive)")
    print(f"as_of      : {fmt_ts(meta['as_of'])} (slots ending after this are 'matched')")
    print(f"trades     : {len(trades)} ({meta['per_slot']} per slot, seed={meta['seed']})")
    print(f"statuses   : {dict(Counter(t['tradeStatus'] for t in trades))}")
    print(f"sellers    : {dict(Counter(t['sellerId'] for t in trades))}")
    print(f"buyers     : {dict(Counter(t['buyerId'] for t in trades))}")
    residuals = sum(1 for t in trades if t["residualBidId"] or t["residualOfferId"])
    print(f"residuals  : {residuals}")
    qty = [t["tradeQuantity"] for t in trades]
    price = [t["tradePrice"] for t in trades]
    print(f"quantity   : min={min(qty)} max={max(qty)} kWh")
    print(f"price      : min={min(price)} max={max(price)} EUR/kWh")
    print(f"market ref : slot {meta['start']} -> {market_id_for_slot(meta['start'])}")
    print(f"sample     : {json.dumps(trades[len(trades) // 2])}")


def load_dataset(path):
    with open(path) as fh:
        data = json.load(fh)
    return data["meta"], data["trades"]


def ensure_dataset(args):
    try:
        meta, trades = load_dataset(args.dataset)
        print(f"Using existing dataset {args.dataset}")
        return meta, trades
    except FileNotFoundError:
        return cmd_generate(args)


# --------------------------------------------------------------------------
# Transport helpers
# --------------------------------------------------------------------------


class Tunnel:
    def __init__(self, args):
        self.args = args
        self.proc = None

    def __enter__(self):
        if self.args.rest_url and self.args.gateway_url:
            return self
        forwards, ports = [], []
        if not self.args.rest_url:
            forwards += ["-L", f"{self.args.local_rest_port}:localhost:8080"]
            ports.append(self.args.local_rest_port)
            self.args.rest_url = f"http://localhost:{self.args.local_rest_port}"
        if not self.args.gateway_url:
            forwards += ["-L", f"{self.args.local_gateway_port}:localhost:3333"]
            ports.append(self.args.local_gateway_port)
            self.args.gateway_url = f"http://localhost:{self.args.local_gateway_port}"
        cmd = ["ssh", "-N", "-o", "ExitOnForwardFailure=yes", "-o", "BatchMode=yes"]
        self.proc = subprocess.Popen(cmd + forwards + [self.args.ssh_host])
        for port in ports:
            wait_for_port(port, self.proc)
        return self

    def __exit__(self, *exc):
        if self.proc:
            self.proc.terminate()
            self.proc.wait(timeout=10)


def wait_for_port(port, proc, timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if proc.poll() is not None:
            sys.exit(f"SSH tunnel exited with code {proc.returncode}")
        with socket.socket() as sock:
            sock.settimeout(1)
            if sock.connect_ex(("127.0.0.1", port)) == 0:
                return
        time.sleep(0.3)
    sys.exit(f"SSH tunnel port {port} did not open within {timeout}s")


def http(method, url, body=None, timeout=60):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read().decode()
    except urllib.error.HTTPError as err:
        return err.code, err.read().decode()


def rest_get_trades(args, start, end):
    query = urllib.parse.urlencode({"start_time": start, "end_time": end})
    status, body = http("GET", f"{args.rest_url}/trades?{query}")
    if status != 200:
        raise RuntimeError(f"GET /trades failed: HTTP {status}: {body[:300]}")
    return json.loads(body)


def mongosh(args, script):
    remote = (
        f"docker exec {MONGO_CONTAINER} sh -c "
        + shlex.quote(
            'mongosh -u "$MONGO_INITDB_ROOT_USERNAME" -p "$MONGO_INITDB_ROOT_PASSWORD" '
            f"--authenticationDatabase admin --quiet {MONGO_DB} --eval "
            + shlex.quote(script)
        )
    )
    out = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", args.ssh_host, remote],
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0:
        raise RuntimeError(f"mongosh failed ({out.returncode}): {out.stderr.strip()}")
    return out.stdout.strip()


def collection_counts(args):
    script = (
        "const r={}; db.getCollectionNames().sort()"
        ".forEach(c=>r[c]=db[c].countDocuments()); print(JSON.stringify(r));"
    )
    return json.loads(mongosh(args, script).splitlines()[-1])


# --------------------------------------------------------------------------
# EWDS round-trip
# --------------------------------------------------------------------------


def is_rate_limited(status, body):
    text = body.lower()
    return status == 429 or "status code 429" in text or "too many requests" in text


def backoff(attempt):
    return min(2.0 * (2 ** min(attempt, 4)), 30.0)


def ewds_trades_query(args, start, end, timeout=90):
    """Publish a trades.query and wait for the matching tradesQueryResponse envelope.

    Returns (envelope, latency_seconds, payload_bytes).
    """
    request_id = f"seed-validate-{int(time.time() * 1000)}-{random.randrange(1 << 30)}"
    envelope = {
        "requestId": request_id,
        "operation": "trades.query",
        "payload": {"startTime": start, "endTime": end},
    }
    message = {
        "fqcn": EWDS_REQUEST_FQCN,
        "topicName": EWDS_REQUEST_TOPIC,
        "topicVersion": EWDS_TOPIC_VERSION,
        "topicOwner": EWDS_TOPIC_OWNER,
        "transactionId": request_id,
        "payload": json.dumps(envelope),
        "anonymousRecipient": [],
    }
    started = time.time()
    attempt = 0
    while True:
        if time.time() - started > timeout:
            raise RuntimeError(f"timeout sending request {request_id}")
        status, body = http("POST", f"{args.gateway_url}/api/v2/messages", message)
        if 200 <= status < 300:
            sent = json.loads(body).get("recipients", {}).get("sent", 0)
            if sent > 0:
                break
            print(f"    gateway delivered to 0 recipients, retrying: {body[:200]}")
        elif not (is_rate_limited(status, body) or status >= 500):
            raise RuntimeError(f"send failed: HTTP {status}: {body[:300]}")
        time.sleep(backoff(attempt))
        attempt += 1

    query = urllib.parse.urlencode(
        {
            "fqcn": EWDS_RESPONSE_FQCN,
            "amount": 100,
            "topicName": EWDS_RESPONSE_TOPIC,
            "topicOwner": EWDS_TOPIC_OWNER,
            "clientId": EWDS_CLIENT_ID,
        }
    )
    attempt = 0
    while time.time() - started <= timeout:
        status, body = http("GET", f"{args.gateway_url}/api/v2/messages?{query}")
        if status == 200:
            attempt = 0
            for msg in json.loads(body) or []:
                try:
                    parsed = json.loads(msg.get("payload", ""))
                except (TypeError, ValueError):
                    continue
                if isinstance(parsed, dict) and parsed.get("requestId") == request_id:
                    return parsed, time.time() - started, len(msg["payload"])
            time.sleep(1.0)
        elif is_rate_limited(status, body) or status >= 500:
            time.sleep(backoff(attempt))
            attempt += 1
        else:
            raise RuntimeError(f"poll failed: HTTP {status}: {body[:300]}")
    raise RuntimeError(f"timeout waiting for response to {request_id}")


def schema_errors(envelope):
    """Hand-written check mirroring int.trades.query.response.v1 + int.trade.schema.v1."""
    errors = []
    allowed = {"requestId", "success", "data", "error"}
    if set(envelope) - allowed:
        errors.append(f"envelope extra keys {set(envelope) - allowed}")
    for key in ("requestId", "success", "data"):
        if key not in envelope:
            errors.append(f"envelope missing '{key}'")
    if not isinstance(envelope.get("success"), bool):
        errors.append("success is not boolean")
    if not isinstance(envelope.get("data"), list):
        return errors + ["data is not an array"]
    err = envelope.get("error")
    if err is not None and not (isinstance(err, dict) and {"code", "message"} <= set(err)):
        errors.append(f"malformed error {err}")
    for item in envelope["data"]:
        tid = item.get("tradeId", "?")
        if set(item) - set(TRADE_KEYS):
            errors.append(f"{tid}: extra keys {set(item) - set(TRADE_KEYS)}")
        if TRADE_REQUIRED - set(item):
            errors.append(f"{tid}: missing keys {TRADE_REQUIRED - set(item)}")
        for key, typ in TRADE_KEYS.items():
            if key in item and (not isinstance(item[key], typ) or isinstance(item[key], bool)):
                errors.append(f"{tid}: {key} has type {type(item[key]).__name__}")
        if item.get("tradeStatus") not in TRADE_STATUSES:
            errors.append(f"{tid}: tradeStatus {item.get('tradeStatus')!r}")
        if not isinstance(item.get("tradeQuantity"), (int, float)) or item["tradeQuantity"] <= 0:
            errors.append(f"{tid}: tradeQuantity not > 0")
        if not isinstance(item.get("timestamp"), int) or item["timestamp"] < 0:
            errors.append(f"{tid}: timestamp not an integer >= 0")
        if len(errors) > 20:
            break
    return errors


# --------------------------------------------------------------------------
# Checks
# --------------------------------------------------------------------------


class Report:
    def __init__(self):
        self.results = []

    def check(self, name, ok, detail=""):
        self.results.append((name, ok))
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}{' - ' + detail if detail else ''}")
        return ok

    def summary(self):
        failed = [name for name, ok in self.results if not ok]
        print(f"\n{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        for name in failed:
            print(f"  FAILED: {name}")
        return not failed


def expected_in(trades, start, end):
    return {t["tradeId"]: t for t in trades if start <= t["timestamp"] < end}


def compare(actual_list, expected):
    actual = {t["tradeId"]: t for t in actual_list}
    missing = expected.keys() - actual.keys()
    extra = actual.keys() - expected.keys()
    differing = [k for k in expected.keys() & actual.keys() if actual[k] != expected[k]]
    problems = []
    if len(actual) != len(actual_list):
        problems.append(f"{len(actual_list) - len(actual)} duplicate ids")
    if missing:
        problems.append(f"{len(missing)} missing (e.g. {sorted(missing)[:3]})")
    if extra:
        problems.append(f"{len(extra)} unexpected (e.g. {sorted(extra)[:3]})")
    if differing:
        k = sorted(differing)[0]
        problems.append(f"{len(differing)} differ (e.g. {actual[k]} != {expected[k]})")
    return problems


def ewds_window_check(report, args, trades, label, start, end):
    try:
        env, latency, size = ewds_trades_query(args, start, end)
    except RuntimeError as err:
        return report.check(f"EWDS {label}", False, str(err))
    errors = schema_errors(env)
    report.check(f"EWDS {label} schema", not errors, "; ".join(errors[:5]))
    if not env.get("success"):
        return report.check(f"EWDS {label}", False, f"success=false error={env.get('error')}")
    problems = compare(env.get("data", []), expected_in(trades, start, end))
    return report.check(
        f"EWDS {label}",
        not problems,
        f"{len(env.get('data', []))} trades, {latency:.1f}s, {size} bytes, "
        f"requestId={env.get('requestId')}" + ("; " + "; ".join(problems) if problems else ""),
    )


def ewds_rejection_check(report, args, label, start, end, code):
    try:
        env, latency, _ = ewds_trades_query(args, start, end)
    except RuntimeError as err:
        return report.check(f"EWDS reject {label}", False, str(err))
    err = env.get("error") or {}
    ok = env.get("success") is False and env.get("data") == [] and err.get("code") == code
    return report.check(
        f"EWDS reject {label}",
        ok,
        f"{latency:.1f}s, success={env.get('success')}, error={err}",
    )


# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------


def cmd_generate(args):
    start, end = parse_date(args.start), parse_date(args.end)
    as_of = int(time.time()) if args.as_of is None else int(args.as_of)
    trades = generate_dataset(start, end, args.trades_per_slot, args.seed, as_of)
    meta = {
        "start": start,
        "end": end,
        "per_slot": args.trades_per_slot,
        "seed": args.seed,
        "as_of": as_of,
        "community_id": COMMUNITY_ID,
    }
    with open(args.dataset, "w") as fh:
        json.dump({"meta": meta, "trades": trades}, fh)
    print(f"Wrote {len(trades)} trades to {args.dataset}")
    dataset_stats(meta, trades)
    return meta, trades


def cmd_counts(args):
    print(json.dumps(collection_counts(args), indent=2))


def cmd_cleanup(args):
    script = (
        f"const r=db.trades.deleteMany({{trade_uuid: /^{TRADE_PREFIX}/}});"
        "print(JSON.stringify({deleted: r.deletedCount}));"
    )
    print(mongosh(args, script).splitlines()[-1])


def cmd_preflight(args):
    meta, _ = ensure_dataset(args)
    report = Report()
    with Tunnel(args):
        start = meta["start"]
        print(f"EWDS preflight: trades.query {fmt_ts(start)} .. {fmt_ts(start + DAY_SECS)}")
        try:
            env, latency, size = ewds_trades_query(args, start, start + DAY_SECS)
        except RuntimeError as err:
            report.check("EWDS preflight round-trip", False, str(err))
            return report.summary()
        errors = schema_errors(env)
        report.check("EWDS preflight schema", not errors, "; ".join(errors[:5]))
        dummies = [t for t in env.get("data", []) if t.get("tradeId", "").startswith(TRADE_PREFIX)]
        report.check(
            "EWDS preflight round-trip",
            env.get("success") is True and not dummies,
            f"success={env.get('success')}, {len(env.get('data', []))} trades, "
            f"{latency:.1f}s, {size} bytes, requestId={env.get('requestId')}",
        )
    return report.summary()


def cmd_seed(args):
    meta, trades = ensure_dataset(args)
    with Tunnel(args):
        existing = [
            t
            for t in rest_get_trades(args, meta["start"], meta["end"])
            if t["tradeId"].startswith(TRADE_PREFIX)
        ]
        if existing:
            if not args.reset:
                sys.exit(f"{len(existing)} dummy trades already exist; rerun with --reset")
            print(f"--reset: removing {len(existing)} existing dummy trades")
            cmd_cleanup(args)
        before = collection_counts(args)
        print(f"Collection counts before seeding: {before}")
        inserted = 0
        for i in range(0, len(trades), args.batch_size):
            batch = trades[i : i + args.batch_size]
            status, body = http("POST", f"{args.rest_url}/trades", batch)
            if status != 200:
                sys.exit(f"batch {i // args.batch_size}: HTTP {status}: {body[:300]}")
            count = len(json.loads(body))
            if count != len(batch):
                sys.exit(f"batch {i // args.batch_size}: inserted {count}/{len(batch)}")
            inserted += count
            if (i // args.batch_size) % 20 == 0:
                print(f"  inserted {inserted}/{len(trades)}")
        print(f"Seeded {inserted} trades")
        with open(args.dataset + ".counts-before.json", "w") as fh:
            json.dump(before, fh)


def cmd_validate(args):
    meta, trades = load_dataset(args.dataset)
    start, end = meta["start"], meta["end"]
    report = Report()
    with Tunnel(args):
        print("V1 REST full coverage (per day)")
        rest_all = []
        day_problems = []
        for day in range(start, end, DAY_SECS):
            got = rest_get_trades(args, day, day + DAY_SECS)
            rest_all.extend(got)
            problems = compare(got, expected_in(trades, day, day + DAY_SECS))
            if problems:
                day_problems.append(f"{fmt_ts(day)}: {'; '.join(problems)}")
        report.check(
            "REST per-day equality",
            not day_problems,
            f"{len(rest_all)} trades over {(end - start) // DAY_SECS} days"
            + ("; " + " | ".join(day_problems[:3]) if day_problems else ""),
        )
        report.check("REST total count", len(rest_all) == len(trades), f"{len(rest_all)}/{len(trades)}")
        per_slot = Counter(t["timestamp"] for t in rest_all)
        bad_slots = [s for s in range(start, end, SLOT_SECS) if per_slot.get(s) != meta["per_slot"]]
        report.check(f"{meta['per_slot']} trades per slot", not bad_slots, f"{len(bad_slots)} bad slots")
        actors = {t["buyerId"] for t in rest_all} | {t["sellerId"] for t in rest_all}
        report.check("actors within roster", actors <= ROSTER, f"{sorted(actors)}")
        self_trades = [t["tradeId"] for t in rest_all if t["buyerId"] == t["sellerId"]]
        report.check("buyer != seller", not self_trades, f"{len(self_trades)} self trades")
        bad_markets = [t["tradeId"] for t in rest_all if t["marketId"] != market_id_for_slot(t["timestamp"])]
        report.check("marketId derivation", not bad_markets, f"{len(bad_markets)} mismatches")
        markets_per_slot = defaultdict(set)
        for t in rest_all:
            markets_per_slot[t["timestamp"]].add(t["marketId"])
        report.check(
            "one market per slot",
            all(len(m) == 1 for m in markets_per_slot.values()),
        )

        print("V2 EWDS tradesQuery -> tradesQueryResponse")
        mid = start + (end - start) // 2
        slot = mid - mid % SLOT_SECS
        ewds_window_check(report, args, trades, f"slot {fmt_ts(slot)}", slot, slot + SLOT_SECS)
        ewds_window_check(
            report, args, trades, f"next slot {fmt_ts(slot + SLOT_SECS)}",
            slot + SLOT_SECS, slot + 2 * SLOT_SECS,
        )
        today = int(time.time()) // DAY_SECS * DAY_SECS
        days = [start, parse_date("2026-08-31"), parse_date("2026-09-01"),
                parse_date("2026-09-15"), today, end - DAY_SECS]
        if args.exhaustive:
            days = list(range(start, end, DAY_SECS))
        for day in sorted({d for d in days if start <= d < end}):
            ewds_window_check(report, args, trades, f"day {fmt_ts(day)[:10]}", day, day + DAY_SECS)
        ewds_window_check(
            report, args, trades, "day before range", start - DAY_SECS, start
        )

        if args.check_range_limit:
            print("V4/V5 EWDS tradesQuery range limit")
            ewds_rejection_check(report, args, "86401s span", start, start + DAY_SECS + 1,
                                 "TIME_RANGE_TOO_LARGE")
            ewds_rejection_check(report, args, "full range", start, end, "TIME_RANGE_TOO_LARGE")
            ewds_rejection_check(report, args, "inverted range", start + DAY_SECS, start,
                                 "INVALID_TIME_RANGE")
            ewds_window_check(report, args, trades, "exactly 1 day (limit)", start, start + DAY_SECS)

        print("V3 isolation")
        counts = collection_counts(args)
        try:
            with open(args.dataset + ".counts-before.json") as fh:
                before = json.load(fh)
        except FileNotFoundError:
            before = None
        others = {c: n for c, n in counts.items() if c != "trades"}
        if before is None:
            report.check("other collections empty", all(n == 0 for n in others.values()), f"{others}")
        else:
            changed = {c: (before.get(c), n) for c, n in others.items() if before.get(c) != n}
            report.check("other collections unchanged", not changed, f"{changed or others}")
        report.check("trades collection count", counts.get("trades") == len(trades),
                     f"{counts.get('trades')}")
    return report.summary()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["generate", "preflight", "seed", "validate", "cleanup", "counts"])
    parser.add_argument("--start", default="2026-08-01", help="first day (UTC, inclusive)")
    parser.add_argument("--end", default="2026-10-01", help="end day (UTC, exclusive)")
    parser.add_argument("--trades-per-slot", type=int, default=2)
    parser.add_argument("--seed", type=int, default=20260801)
    parser.add_argument("--as-of", type=int, help="epoch cutoff for 'matched' status (default: now)")
    parser.add_argument("--dataset", default="dummy_trades_dataset.json")
    parser.add_argument("--batch-size", type=int, default=100)
    parser.add_argument("--reset", action="store_true", help="seed: delete existing dummy trades first")
    parser.add_argument("--exhaustive", action="store_true", help="validate: EWDS query every day")
    parser.add_argument("--check-range-limit", action="store_true",
                        help="validate: also check the 1-day tradesQuery limit (Part B)")
    parser.add_argument("--ssh-host", default="root@167.233.109.37")
    parser.add_argument("--rest-url", help="skip the tunnel for REST, e.g. http://localhost:8080")
    parser.add_argument("--gateway-url", help="skip the tunnel for the gateway")
    parser.add_argument("--local-rest-port", type=int, default=18080)
    parser.add_argument("--local-gateway-port", type=int, default=13333)
    args = parser.parse_args()

    if args.trades_per_slot > len(LOADS):
        sys.exit(f"--trades-per-slot must be <= {len(LOADS)} (distinct buyers per slot)")

    result = {
        "generate": cmd_generate,
        "preflight": cmd_preflight,
        "seed": cmd_seed,
        "validate": cmd_validate,
        "cleanup": cmd_cleanup,
        "counts": cmd_counts,
    }[args.command](args)
    if result is False:
        sys.exit(1)


if __name__ == "__main__":
    main()
