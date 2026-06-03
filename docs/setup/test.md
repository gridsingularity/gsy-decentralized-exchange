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
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit e2e-tests
```

### EWDS Transport E2E

Run E2E through a local DDHub Client Gateway connected to the EWF-hosted EWC broker/cache services. This requires a Switchboard-enrolled DID private key, active IAM roles, mTLS configured on the gateway, and the GSY request/response topics/channels created in the gateway before the tests can exchange messages.

```bash
cp .env.ewds.local.example .env.ewds.local
# Configure the DID/private key and upload mTLS material through http://localhost:3009 first.
# Restart docker-compose.ewds.yml without -v after the UI setup so scheduler/API reload Vault state.
```

If the EWDS gateway stack is already running and healthy in the same compose
project, do not run `down` on the combined compose files because that also stops
the gateway, Vault, and Postgres containers. Reset only the GSY/e2e containers
when a clean e2e service run is needed:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.test.yml \
  -f docker-compose.ewds.yml \
  --profile ewds \
  stop e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator gsy-contracts-bootstrap anvil mongodb

docker compose --env-file .env.ewds.local \
  -f docker-compose.test.yml \
  -f docker-compose.ewds.yml \
  --profile ewds \
  rm -f e2e-tests gsy-offchain-storage gsy-matching-engine gsy-execution-engine gsy-community-client gsy-market-orchestrator gsy-contracts-bootstrap anvil mongodb
```

Final validated EWDS e2e command:

```bash
docker compose --env-file .env.ewds.local \
  -f docker-compose.test.yml \
  -f docker-compose.ewds.yml \
  --profile ewds \
  up --build --abort-on-container-exit e2e-tests
```

This command was validated with the local DDHub Client Gateway connected to the
EWF-hosted broker and the following local channels/topics:

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

Keep these timeout settings in `.env.ewds.local` for deterministic runs over
the asynchronous DDHub broker path:

```bash
EWDS_RESPONSE_TIMEOUT_MS=60000
EWDS_RESPONSE_POLL_INTERVAL_MS=1000
EWDS_HANDLER_POLL_INTERVAL_MS=500
EWDS_HANDLER_BATCH_SIZE=100
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
- `EWDS_GATEWAY_PLATFORM` (set `linux/amd64` on Apple Silicon when using current EWDS images)

Current e2e suite validates:

- Standard bid/offer matching and on-chain settlement.
- Preference-based matching behavior and preferred price selection.
- Penalty submission from execution engine.
- EWDS request/response transport for order reads through the local Client Gateway.
