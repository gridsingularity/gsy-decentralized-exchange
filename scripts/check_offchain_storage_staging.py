#!/usr/bin/env python3
"""Check that the off-chain storage on staging answers EWDS queries.

For the orders, trades, measurements, markets, communities and facilities queries of the
off-chain storage's EWDS request handler, the script publishes a request through the
staging client gateway and waits for the response with the same requestId, the same way
the other GSY services query the off-chain storage.

Usage:
    STAGING_IP=1.2.3.4 python3 scripts/check_offchain_storage_staging.py

Environment variables:
    STAGING_IP                    IP of the staging server (required)
    EWDS_GATEWAY_PORT             Client gateway API port (default: 3333)
    EWDS_GATEWAY_SCHEME           http or https (default: http)
    EWDS_REQUEST_PUBLISH_FQCN     Channel the requests are published on
                                  (default: gsy.intelligent.requests.pub)
    EWDS_RESPONSE_SUBSCRIBE_FQCN  Channel the responses are read from
                                  (default: gsy.intelligent.responses.sub)
    EWDS_TOPIC_OWNER              (default: integration.apps.intelligent.auth.ewc)
    EWDS_TOPIC_VERSION            (default: 1.0.0)
    EWDS_TOPIC_SUFFIX             Appended to the request topics, e.g. "Test" for
                                  ordersQueryTest (default: none)
    EWDS_CHECK_CLIENT_ID          Gateway cursor prefix for the response polls. No service may
                                  use it, or the two split the responses between them
                                  (default: gsystagingcheck)
    RESPONSE_TIMEOUT              Seconds to wait for each response (default: 60). The handler
                                  skips requests older than EWDS_REQUEST_MAX_AGE_MS (60 s on
                                  staging).
    LOOKBACK_SECONDS              Time window of the orders, trades, measurements and markets
                                  queries, at most 86400 because of the trades.query limit
                                  (default: 3600)

Requires requests (pip install requests).
Exit code: 0 if every query was answered with success, 1 otherwise, 2 on usage errors.
"""

import json
import os
import re
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone

import requests

HTTP_TIMEOUT = 90  # The first poll of a new client ID can take more than 30 s.
POLL_INTERVAL = 2
BATCH_SIZE = 100
MAX_DRAIN_POLLS = 50
MAX_ATTEMPTS = 4

if sys.stdout.isatty():
    GREEN, RED, DIM, RESET = "\033[32m", "\033[31m", "\033[2m", "\033[0m"
else:
    GREEN = RED = DIM = RESET = ""


def usage_error(message):
    print(message, file=sys.stderr)
    sys.exit(2)


def env_int(name, default):
    value = os.environ.get(name) or str(default)
    try:
        return int(value)
    except ValueError:
        usage_error(f"{name} must be an integer, got {value!r}")


def alphanumeric(value):
    """The gateway accepts only alphanumeric client IDs."""
    return re.sub(r"[^A-Za-z0-9]", "", value)


def rfc3339(epoch):
    return datetime.fromtimestamp(epoch, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class GatewayError(Exception):
    pass


class Gateway:
    def __init__(self, base_url, request_fqcn, response_fqcn, topic_owner, topic_version, client_id):
        self.base_url = base_url
        self.request_fqcn = request_fqcn
        self.response_fqcn = response_fqcn
        self.topic_owner = topic_owner
        self.topic_version = topic_version
        self.client_id = client_id
        self.session = requests.Session()
        self.session.headers["Accept"] = "application/json"

    def call(self, method, path, params=None, body=None):
        """Calls the gateway API and returns the response. Retries rate limits (also the
        broker's 429 wrapped in a 400) and 5xx a few times."""
        url = f"{self.base_url}{path}"
        delay = 2
        for attempt in range(1, MAX_ATTEMPTS + 1):
            try:
                response = self.session.request(
                    method, url, params=params, json=body, timeout=HTTP_TIMEOUT
                )
            except requests.RequestException as error:
                raise GatewayError(f"{method} {url} failed: {error}")
            if response.ok:
                return response
            status = response.status_code
            retryable = status == 429 or status >= 500 or "status code 429" in response.text.lower()
            if attempt < MAX_ATTEMPTS and retryable:
                time.sleep(delay)
                delay *= 2
                continue
            raise GatewayError(f"HTTP {status}: {response.text[:300]}")

    def check_channel(self, fqcn):
        self.call("GET", f"/channels/{fqcn}")

    def publish_request(self, topic, request_id, operation, payload):
        """Publishes a request and returns how many recipients the gateway delivered it to."""
        envelope = {"requestId": request_id, "operation": operation, "payload": payload}
        message = {
            "fqcn": self.request_fqcn,
            "topicName": topic,
            "topicVersion": self.topic_version,
            "topicOwner": self.topic_owner,
            "transactionId": request_id,
            "payload": json.dumps(envelope),
            "anonymousRecipient": [],
        }
        response = self.call("POST", "/messages", body=message)
        try:
            return response.json().get("recipients", {}).get("sent", 0), response.text
        except (ValueError, AttributeError):
            return 0, response.text

    def poll_responses(self, topic):
        """One read of a response topic with this script's cursor for it."""
        response = self.call(
            "GET",
            "/messages",
            params={
                "fqcn": self.response_fqcn,
                "amount": BATCH_SIZE,
                "topicName": topic,
                "topicOwner": self.topic_owner,
                "clientId": alphanumeric(self.client_id + topic),
            },
        )
        try:
            messages = response.json()
        except ValueError:
            messages = None
        if not isinstance(messages, list):
            raise GatewayError(f"the gateway returned no message list: {response.text[:300]}")
        return messages


@dataclass
class Check:
    operation: str
    topic: str
    payload: object
    request_id: str = ""
    published_at: float = 0.0
    # (passed, detail) once the check has a result.
    result: tuple = None

    @property
    def response_topic(self):
        return f"{self.topic}Response"


def find_response(messages, request_id):
    """The (sender, response envelope) for `request_id`. Other requesters' responses share
    the topic, so ours is picked by requestId."""
    for message in messages:
        try:
            envelope = json.loads(message.get("payload", ""))
        except (TypeError, ValueError, AttributeError):
            continue
        if isinstance(envelope, dict) and envelope.get("requestId") == request_id:
            return message.get("sender") or "unknown sender", envelope
    return None


def report(check):
    passed, detail = check.result
    label = f"{GREEN}[PASS]{RESET}" if passed else f"{RED}[FAIL]{RESET}"
    print(f"{label} {check.operation:<28} {detail}")


def await_response(gateway, check, timeout):
    """Reads the check's response topic and sets its result once the response has arrived or
    the timeout has passed."""
    while True:
        try:
            messages = gateway.poll_responses(check.response_topic)
        except GatewayError as error:
            check.result = (False, f"polling failed: {error}")
            return
        found = find_response(messages, check.request_id)
        if found:
            sender, envelope = found
            elapsed = time.monotonic() - check.published_at
            if envelope.get("success") is True:
                records = len(envelope.get("data") or [])
                check.result = (True, f"{records} records in {elapsed:.1f}s {DIM}from {sender}{RESET}")
            else:
                error = envelope.get("error") or {}
                check.result = (
                    False,
                    f"{error.get('code', '?')}: {error.get('message', 'no message')} {DIM}from {sender}{RESET}",
                )
            return
        # A full batch means more responses are waiting: read on at once.
        if len(messages) < BATCH_SIZE:
            break
    if time.monotonic() - check.published_at >= timeout:
        check.result = (False, f"no response within {timeout}s (request {check.request_id})")


def main():
    staging_ip = os.environ.get("STAGING_IP")
    if not staging_ip:
        usage_error(f"Set STAGING_IP to the staging server IP, e.g. STAGING_IP=1.2.3.4 {sys.argv[0]}")

    scheme = os.environ.get("EWDS_GATEWAY_SCHEME") or "http"
    port = env_int("EWDS_GATEWAY_PORT", 3333)
    timeout = env_int("RESPONSE_TIMEOUT", 60)
    lookback = env_int("LOOKBACK_SECONDS", 3600)
    topic_suffix = os.environ.get("EWDS_TOPIC_SUFFIX", "")
    gateway = Gateway(
        base_url=f"{scheme}://{staging_ip}:{port}/api/v2",
        request_fqcn=os.environ.get("EWDS_REQUEST_PUBLISH_FQCN") or "gsy.intelligent.requests.pub",
        response_fqcn=os.environ.get("EWDS_RESPONSE_SUBSCRIBE_FQCN") or "gsy.intelligent.responses.sub",
        topic_owner=os.environ.get("EWDS_TOPIC_OWNER") or "integration.apps.intelligent.auth.ewc",
        topic_version=os.environ.get("EWDS_TOPIC_VERSION") or "1.0.0",
        client_id=alphanumeric(os.environ.get("EWDS_CHECK_CLIENT_ID") or "gsystagingcheck"),
    )

    now = int(time.time())
    time_range = {"startTime": rfc3339(now - lookback), "endTime": rfc3339(now)}
    checks = [
        Check(operation, topic + topic_suffix, payload)
        for operation, topic, payload in [
            ("orders.query", "ordersQuery", time_range),
            ("trades.query", "tradesQuery", time_range),
            ("measurements.query", "measurementsQuery", time_range),
            ("markets.query", "marketsQuery", time_range),
            ("communities.query", "communitiesQuery", {}),
            ("facilities.query", "facilitiesQuery", {}),
        ]
    ]

    print(f"Gateway:     {gateway.base_url}")
    print(f"Channels:    {gateway.request_fqcn} -> {gateway.response_fqcn}")
    print(f"Time window: {time_range['startTime']} .. {time_range['endTime']}")
    print()

    print("== Gateway channels")
    channels_ok = True
    for fqcn in (gateway.request_fqcn, gateway.response_fqcn):
        try:
            gateway.check_channel(fqcn)
            print(f"{GREEN}[PASS]{RESET} {fqcn:<28} exists")
        except GatewayError as error:
            print(f"{RED}[FAIL]{RESET} {fqcn:<28} {error}")
            channels_ok = False
            break
    if not channels_ok:
        print()
        print(f"{RED}The gateway or its channels are not available; skipping the queries.{RESET}")
        return 1
    print()

    # Skip the responses already waiting for this script's cursors, so that only the
    # responses to this run's requests are left to read.
    print(f"== Catching up the response cursors {DIM}(can take minutes on the first run){RESET}")
    skipped = 0
    for check in checks:
        try:
            for _ in range(MAX_DRAIN_POLLS):
                messages = gateway.poll_responses(check.response_topic)
                skipped += len(messages)
                if len(messages) < BATCH_SIZE:
                    break
        except GatewayError as error:
            check.result = (False, f"polling {check.response_topic} failed: {error}")
    print(f"Skipped {skipped} older responses")
    print()

    print("== Publishing requests")
    for check in checks:
        if check.result:
            continue
        request_id = f"healthcheck-{re.sub(r'[._]', '-', check.operation)}-{now}-{os.getpid()}"
        try:
            sent, body = gateway.publish_request(check.topic, request_id, check.operation, check.payload)
        except GatewayError as error:
            check.result = (False, f"publish failed: {error}")
            continue
        if sent < 1:
            check.result = (False, f"published to no recipient: {body[:300]}")
            continue
        check.request_id = request_id
        check.published_at = time.monotonic()
        print(f"{DIM}{check.operation:<28} {request_id} on {check.topic}{RESET}")
    print()

    print(f"== Responses {DIM}(timeout {timeout}s){RESET}")
    for check in checks:
        if check.result:
            report(check)
    # Read the open checks' topics in turns, so that a missing response doesn't delay the others.
    open_checks = [check for check in checks if not check.result]
    while open_checks:
        for check in open_checks:
            await_response(gateway, check, timeout)
            if check.result:
                report(check)
        open_checks = [check for check in open_checks if not check.result]
        if open_checks:
            time.sleep(POLL_INTERVAL)
    print()

    passed = sum(1 for check in checks if check.result[0])
    failed = len(checks) - passed
    print(f"Summary: {passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
