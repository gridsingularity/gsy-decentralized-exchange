# Testing

## Contract Tests

```bash
cd gsy-contracts
npm install
npx hardhat test
```

## Rust Integration Tests

Run per component:

```bash
cargo test --manifest-path gsy-market-orchestrator/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-matching-engine/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-execution-engine/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-offchain-storage/Cargo.toml --test api
cargo test --manifest-path gsy-community-client/Cargo.toml --tests
```

## End-to-End Cucumber Tests

```bash
./scripts/contracts.sh local deploy

docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

The default `MATCHING_ALGORITHM=pay_as_bid` run executes the features under
`e2e-tests/features/pay_as_bid`. To run the isolated two-sided pay-as-clear
scenario, set the same value for the matching engine and e2e runner through
Compose:

```bash
MATCHING_ALGORITHM=pay_as_clear \
docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

The E2E runner selects `features/pay_as_clear` from this value and verifies
that the cumulative curves stop where the next bid drops below the next offer,
accepted trades share the marginal accepted-offer clearing price, and both
orders beyond that crossing remain open. It also verifies that every accepted
trade is settled on-chain and receives a non-zero execution-engine penalty.
`pay_as_clear` defaults to a `64`-block matching interval so all scenario
orders are collected before one clearing cycle. Override it with a positive
`MATCHING_ENGINE_BLOCK_INTERVAL` value only when both the matching engine and
E2E runner use the same value, as they do through the Compose configuration.
After the complete order book is indexed, the E2E harness fast-forwards the
local Anvil chain with empty-block RPC calls rather than waiting for one
transaction per block.

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

Start and validate the gateway first:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.ewds.yml \
  up --build
```

After the EWDS gateway stack is running and healthy in the same compose project,
deploy the local contracts stack, then start the GSY/e2e stack from the normal
test compose file. Do not run `down` on `docker-compose.ewds.yml` unless you
intentionally want to stop the gateway, Vault, and Postgres containers. Reset
only the GSY/e2e containers when a clean e2e service run is needed:

Run both compose commands from the repository root without changing the Compose
project name so the GSY containers can resolve `ddhub-gateway-api` on the shared
default Docker network. The same rule applies to `docker-compose.contracts.yml`;
the local `anvil` container must be on the same default network.

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.test.yml \
  stop e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator mongodb

docker compose --env-file .env.ewds.local \
  -f docker-compose.test.yml \
  rm -f e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator mongodb
```

Final validated EWDS e2e command:

```bash
./scripts/contracts.sh local deploy

docker compose --env-file .env.ewds.local \
  --env-file contracts-output/addresses.env \
  -f docker-compose.test.yml \
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
  -f docker-compose.test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

The EWDS request/reply transport was previously validated with the local DDHub
Client Gateway connected to the EWF-hosted broker and the following local
channels/topics:

- `gsy.intelligent.requests.pub` / `gsy.intelligent.requests.sub`
- `gsy.intelligent.responses.pub` / `gsy.intelligent.responses.sub`
- `ordersQuery` / `ordersQueryResponse`
- `tradesQuery` / `tradesQueryResponse`
- `measurementsQuery` / `measurementsQueryResponse`

Expected passing summary:

```text
2 features
2 scenarios (2 passed)
20 steps (20 passed)
```

The isolated pay-as-clear run has this expected summary:

```text
1 feature
1 scenario (1 passed)
11 steps (11 passed)
```

The pay-as-clear command runs one dedicated feature and scenario.

If the Client Gateway returns HTTP `400` with a nested broker `status code 429`,
the request was rate-limited before it reached the matching algorithm. Validate
the algorithm first with the local HTTP command above, then retry the EWDS path.
The clients use exponential backoff for these wrapped rate-limit responses.

Keep these timeout settings in `.env.ewds.local` for deterministic runs over
the asynchronous DDHub broker path:

```bash
EWDS_RESPONSE_TIMEOUT_MS=60000
EWDS_RESPONSE_POLL_INTERVAL_MS=1000
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
- `EWDS_HANDLER_POLL_INTERVAL_MS`
- `EWDS_HANDLER_BATCH_SIZE`
- `EWDS_RATE_LIMIT_BACKOFF_MS`
- `EWDS_RATE_LIMIT_MAX_BACKOFF_MS`
- `EWDS_GATEWAY_PLATFORM` (set `linux/amd64` on Apple Silicon when using current EWDS images)

Current e2e suite validates:

- Standard bid/offer matching and on-chain settlement.
- Preference-based matching behavior and preferred price selection.
- Two-sided pay-as-clear matching, including curve crossing, uniform price,
  and uncleared bid/offer assertions.
- Penalty submission from execution engine.
- EWDS request/response transport for order reads through the local Client Gateway.
