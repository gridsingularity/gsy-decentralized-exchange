# Testing

## Integration & Unit Tests (consolidated)

All Rust integration tests, the contract tests, and the listener test run
through a single Compose file and a wrapper script. The script runs every
service sequentially, continues past failures, and prints a summary listing
each test with its runtime — passed and failed separately. It exits non-zero
if any test failed, so CI marks the job red.

```bash
./run_integration_tests.sh
```

Services executed (in order):

- `gsy-listener-test`
- `gsy-offchain-storage-integration-test` (starts `mongodb`)
- `gsy-market-orchestrator-integration-test`
- `gsy-matching-engine-integration-test`
- `gsy-execution-engine-integration-test`
- `gsy-community-client-integration-test`
- `gsy-primitives-integration-test`
- `gsy-contracts-tests`

The script tears down containers and volumes on exit (`docker compose down -v`),
including on failure. When run under GitHub Actions it also appends the
results to the run's step summary.

### Base image

Every Rust test image builds `FROM ghcr.io/gridsingularity/gsy-rust-base:latest`,
which contains the build toolchain and all third-party dependencies
precompiled (see `Dockerfile.base`). Before running any test, the script makes
sure this image is available:

1. If the image exists locally for the machine's architecture, it is used as is.
2. Otherwise the script pulls it from GHCR (this is what happens in CI).
3. If the pull fails, e.g. locally without GHCR access, or returns an image
   for a different architecture, the script builds it from `Dockerfile.base`.
   The first build takes a while.

The published image is `linux/amd64` only, for the GitHub runners. On Apple
Silicon Macs the script therefore always builds a native arm64 base image
locally the first time.

The script does not check whether a local copy is up to date. After changing
`Cargo.toml`, `Cargo.lock` or `Dockerfile.base`, rebuild it so the service
builds can reuse the precompiled dependencies again:

```bash
docker build -f Dockerfile.base -t ghcr.io/gridsingularity/gsy-rust-base:latest .
```

A stale base image never breaks a build: dependencies missing from it are
compiled during the service build, which is only slower.

### Running a single test

Running a service directly skips the script's base image check, so the base
image must already exist locally or be pullable (see above).

Target one service directly instead of the whole suite:

```bash
docker compose -f docker-compose.integration.yml run --rm --build \
  gsy-matching-engine-integration-test
```

If the full dev stack is already up, isolate the run with a project name to
avoid `mongodb` container/port (27017) clashes:

```bash
docker compose -p integration -f docker-compose.integration.yml run --rm --build \
  gsy-matching-engine-integration-test
```

### CI

The suite runs on every pull request in GitHub Actions via
`.github/workflows/integration_tests.yml`:

```yaml
- name: Log in to GHCR
  uses: docker/login-action@v4
  with:
    registry: ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets.GITHUB_TOKEN }}

- name: Run integration tests
  run: ./run_integration_tests.sh
```

The GHCR login lets the script pull the published base image instead of
building it. The image is published by the `base-image` job in
`.github/workflows/deploy_staging.yml` on every push to `GSYDEXv2` (e.g. a
merged PR). The staging service builds wait for that job, so they always use
the freshly published base image. Docker and Compose are preinstalled on
`ubuntu-latest`. The script's per-test pass/fail list with runtimes appears in
the run's summary panel.

## End-to-End Cucumber Tests

```bash
./scripts/contracts.sh local deploy

docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

`docker-compose.e2e-test.yml` builds the base image itself from
`Dockerfile.base` (through the `gsy-rust-base` service), so no GHCR access is
needed for e2e runs.

The default `MATCHING_ALGORITHM=pay_as_bid` run executes the features under
`e2e-tests/features/pay_as_bid`. To run the two-sided pay-as-clear scenarios,
set the same value for the matching engine and e2e runner through
Compose:

```bash
MATCHING_ALGORITHM=pay_as_clear \
docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

The E2E runner selects `features/pay_as_clear` from this value. The first
scenario verifies that cumulative curves stop where the next bid drops below
the next offer, accepted trades share the marginal accepted-offer clearing
price, and both orders beyond that crossing remain open. The combined scenario
also verifies that a preferred bilateral trade keeps its negotiated rate while
the remaining standard order book clears at one uniform price. Both scenarios
verify on-chain settlement and non-zero execution-engine penalties for every
accepted trade.
`pay_as_clear` defaults to a `64`-block matching interval so all scenario
orders are collected before one clearing cycle. Override it with a positive
`MATCHING_ENGINE_BLOCK_INTERVAL` value only when both the matching engine and
E2E runner use the same value, as they do through the Compose configuration.
If the current interval does not have enough capacity for the complete order
book, the E2E harness first advances local Anvil to the next matching boundary.
After the order book is indexed, it fast-forwards to the clearing boundary with
empty-block RPC calls rather than waiting for one transaction per block.

Before waiting for the orchestrator, the E2E runner idempotently upserts a
canonical community using `OFFCHAIN_STORAGE_TRANSPORT`. The orchestrator then
queries the same community collection and opens the community-aware Spot market
whose ID is derived from community UUID, market type, and delivery slot.

The contracts command starts the dedicated local Anvil container, deploys the
upgradeable contract suite, grants service roles, and writes
`contracts-output/addresses.env`. Keep that Anvil container running while the
e2e compose stack executes.

### EWDS Transport E2E

Run E2E through a local DDHub Client Gateway connected to the EWF-hosted EWC broker/cache services. This requires a Switchboard-enrolled DID private key, active IAM roles, mTLS configured on the gateway, and the GSY request/response topics/channels created in the gateway before the tests can exchange messages.

```bash
cp .env.ewds.local.example .env.ewds.local
# Configure the DID/private key and upload mTLS material through http://localhost:3009 first.
# Restart docker-compose.ewds.yml without -v after the UI setup so scheduler/API reload Vault state.
```

### Validated EWDS E2E Sequence

1. Start the local DDHub Client Gateway stack and keep it running:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.ewds.yml \
  up --build
```

Before running the GSY services, verify the gateway is connected to EWDS and the
required local gateway channels/topics are configured.

2. In a separate shell, deploy the local contracts stack:

```bash
./scripts/contracts.sh local deploy
```

This starts the dedicated local `anvil` container, deploys the upgradeable
contract suite, grants service roles, and writes
`contracts-output/addresses.env`. Keep the `anvil` container running while the
e2e compose stack executes.

3. Run the GSY e2e stack against the already-running gateway and local Anvil:

```bash
docker compose --env-file .env.ewds.local \
  --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

Run all commands from the repository root without changing the Compose project
name. The gateway, contracts, and GSY test stacks must share the same default
Docker network so services can resolve `ddhub-gateway-api` and `anvil`.

Do not include `-f docker-compose.ewds.yml` in the e2e command if the gateway
stack is already running. The GSY services only need `.env.ewds.local` for EWDS
client settings and `contracts-output/addresses.env` for deployed contract
addresses.

Use `--force-recreate` on the e2e command only when you explicitly want Docker
to recreate the GSY/e2e containers.

Do not run `down` on `docker-compose.ewds.yml` unless you intentionally want to
stop the gateway, Vault, and Postgres containers. Reset only the GSY/e2e
containers when a clean e2e service run is needed:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.e2e-test.yml \
  stop e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator mongodb

docker compose --env-file .env.ewds.local \
  -f docker-compose.e2e-test.yml \
  rm -f e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator mongodb
```

Final validated EWDS e2e command:

```bash
./scripts/contracts.sh local deploy

docker compose --env-file .env.ewds.local \
  --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

To run the pay-as-clear scenario through the same gateway, prefix the e2e
command with `MATCHING_ALGORITHM=pay_as_clear`. This selects the `64`-block
default aggregation interval for both the matching engine and the E2E runner:

```bash
MATCHING_ALGORITHM=pay_as_clear \
docker compose --env-file .env.ewds.local \
  --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

This sequence was validated with the local DDHub Client Gateway connected to the
EWF-hosted broker and the following local channels/topics:

- `gsy.intelligent.requests.pub` / `gsy.intelligent.requests.sub`
- `gsy.intelligent.responses.pub` / `gsy.intelligent.responses.sub`
- `ordersQuery` / `ordersQueryResponse`
- `tradesQuery` / `tradesQueryResponse`
- `measurementsQuery` / `measurementsQueryResponse`
- `communityUpsert` / `communityUpsertResponse`
- `communitiesQuery` / `communitiesQueryResponse`

Expected passing summary:

```text
3 features
3 scenarios (3 passed)
30 steps (30 passed)
```

The pay-as-clear run has this expected summary:

```text
1 feature
2 scenarios (2 passed)
23 steps (23 passed)
```

The pay-as-clear command runs one dedicated feature with a standard clearing
scenario and a combined preference-plus-standard scenario.

If the Client Gateway returns HTTP `400` with a nested broker `status code 429`,
the request was rate-limited before it reached the matching algorithm. Validate
the algorithm first with the local HTTP command above, then retry the EWDS path.
The clients use exponential backoff for these wrapped rate-limit responses.

Keep these timeout settings in `.env.ewds.local` for deterministic runs over
the asynchronous DDHub broker path:

```bash
EWDS_RESPONSE_TIMEOUT_MS=60000
EWDS_RESPONSE_POLL_INTERVAL_MS=1000
EWDS_EMPTY_RESPONSE_GRACE_MS=10000
EWDS_HANDLER_POLL_INTERVAL_MS=500
EWDS_HANDLER_BATCH_SIZE=100
EWDS_RATE_LIMIT_BACKOFF_MS=2000
EWDS_RATE_LIMIT_MAX_BACKOFF_MS=30000
```

Important EWDS variables for test runs:

- `EWDS_BROKER_BASE_URL`
- `EWDS_CACHE_SERVER_URL`
- `EWDS_EVENT_SERVER_URL`
- `EWDS_RPC_URL` / `EWDS_ENS_URL`
- `EWDS_CHAIN_ID` / `EWDS_CHAIN_NAME`
- `EWDS_PARENT_NAMESPACE`
- `EWDS_DID_REGISTRY_ADDRESS`
- `EWDS_MTLS_ENABLED`
- `OFFCHAIN_STORAGE_TRANSPORT`
- `EWDS_ENABLE_HANDLER`
- `EWDS_RESPONSE_TIMEOUT_MS`
- `EWDS_RESPONSE_POLL_INTERVAL_MS`
- `EWDS_EMPTY_RESPONSE_GRACE_MS`
- `EWDS_HANDLER_POLL_INTERVAL_MS`
- `EWDS_HANDLER_BATCH_SIZE`
- `EWDS_RATE_LIMIT_BACKOFF_MS`
- `EWDS_RATE_LIMIT_MAX_BACKOFF_MS`
- `EWDS_E2E_CLIENT_ID`
- `EWDS_GATEWAY_PLATFORM` (set `linux/amd64` on Apple Silicon when using current EWDS images)

Current e2e suite validates:

- Standard bid/offer matching and on-chain settlement.
- Preference-based matching behavior and preferred price selection.
- Two-sided pay-as-clear matching, including curve crossing, uniform price,
  and uncleared bid/offer assertions.
- Penalty submission from execution engine.
- EWDS request/response transport for order reads through the local Client Gateway.
- Community upsert and orchestrator community discovery through the selected
  HTTP or EWDS transport.
- Community-aware market ID derivation and on-chain market opening.
- Distinct Spot market creation for multiple communities in the same delivery
  slot, with end-to-end verification that matching, settlement, indexing, and
  penalties remain isolated per community market.
