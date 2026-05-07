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
cargo test --manifest-path gsy-orderbook-service/Cargo.toml --test api
cargo test --manifest-path gsy-community-client/Cargo.toml --tests
```

## End-to-End Cucumber Tests

```bash
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit e2e-tests
```

### EWDS Transport E2E

Run E2E through a local DDHub Client Gateway connected to the EWF-hosted EWC broker/cache services. This requires a Switchboard-enrolled DID private key, active IAM roles, and mTLS configured on the gateway before the tests can exchange messages.

```bash
cp .env.ewds.local.example .env.ewds.local
# Configure the DID/private key and upload mTLS material through http://localhost:3009 first.
# Restart docker-compose.ewds.yml without -v after the UI setup so scheduler/API reload Vault state.
docker compose --env-file .env.ewds.local -f docker-compose.test.yml -f docker-compose.ewds.yml --profile ewds up --build --abort-on-container-exit e2e-tests
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
- `EWDS_GATEWAY_PLATFORM` (set `linux/amd64` on Apple Silicon when using current EWDS images)

Current e2e suite validates:

- Standard bid/offer matching and on-chain settlement.
- Preference-based matching behavior and preferred price selection.
- Penalty submission from execution engine.
