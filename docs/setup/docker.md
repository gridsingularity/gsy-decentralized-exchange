# Docker

## Compose Topology

The default compose files start an EVM-centered local DEX environment:

1. `anvil` starts and exposes port `8545`.
2. `gsy-contracts-bootstrap` deploys contracts and grants roles.
3. Service containers start with predefined contract addresses.
4. `gsy-orderbook` subscribes to chain events and exposes APIs.

## Main Commands

```bash
# build all images
docker compose build

# run core stack
docker compose up

# run and rebuild
docker compose up --build

# stop
docker compose down
```

## EWDS Client Gateway Against EWF

Use the EWDS overlay to run a local DDHub Client Gateway against EWF-hosted EWC Digital Spine services. This setup uses the shared EWF broker, public EWC RPC, and public identity cache; it does not run a local broker, IAM chain, or SSI hub.

```bash
cp .env.ewds.local.example .env.ewds.local
# Default local setup uses the bundled HashiCorp Vault service.
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

After the gateway starts, use the local proxy for the UI and API docs. Upload the EWF-provided mTLS client certificate and matching private key through `POST /api/v2/certificate`:

```bash
http://localhost:3009
http://localhost:3009/docs
```

Then configure the DID/EWC wallet private key through the UI. After mTLS and identity are stored, restart the compose stack without removing volumes so the API and scheduler reload the Vault-backed certificate/private-key material consistently:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml down --remove-orphans
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

Do not use `down -v` after mTLS/private-key setup unless you intentionally want to wipe the local Vault and Postgres state.

Expected healthy signals after restart:

- Dashboard shows `DDHub Message Broker` as `Online`.
- Dashboard shows `MTLS STATUS` as `Valid`.
- Scheduler logs include `connected to did registry, iam setup finalized`.
- Scheduler logs include `Login successful` and `Init ext channel successful`.
- Enrolment logs show synced roles for `dsmb.apps.ddhub.energyweb.auth.ewc` and `integration.apps.intelligent.auth.ewc`.

Runtime defaults in `.env.ewds.local.example` match the EWF-shared values:

- `EWDS_BROKER_BASE_URL=https://ddhub-ewc.energyweb.org`
- `EWDS_CACHE_SERVER_URL=https://identitycache.energyweb.org/v1`
- `EWDS_EVENT_SERVER_URL=https://identitycache.energyweb.org`
- `EWDS_RPC_URL=https://rpc.energyweb.org/`
- `EWDS_CHAIN_ID=246`
- `EWDS_CHAIN_NAME=EWC`
- `EWDS_PARENT_NAMESPACE=dsmb.apps.ddhub.energyweb.auth.ewc`
- `EWDS_DID_REGISTRY_ADDRESS=0xE29672f34e92b56C9169f9D485fFc8b9A136BCE4`
- `EWDS_MTLS_ENABLED=true`

### EWDS DEX Overlay

Once the local gateway can connect to the shared broker and EWF has created or authorized the required GSY topics/channels, run the DEX stack with EWDS transport:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.yml -f docker-compose.ewds.yml --profile ewds up --build
```

GSY topic/channel registration is intentionally separate from the local compose file. Run the DEX overlay only after EWF confirms the required IAM roles and topic ownership or provides test topics/channels.

Useful runtime overrides:

- `EWDS_GATEWAY_BACKEND_IMAGE`
- `EWDS_GATEWAY_FRONTEND_IMAGE`
- `EWDS_GATEWAY_SCHEDULER_IMAGE`
- `EWDS_GATEWAY_PLATFORM`
- `EWDS_BROKER_BASE_URL`
- `EWDS_CACHE_SERVER_URL`
- `EWDS_EVENT_SERVER_URL`
- `EWDS_RPC_URL` / `EWDS_ENS_URL`
- `EWDS_CHAIN_ID` / `EWDS_CHAIN_NAME`
- `EWDS_PARENT_NAMESPACE`
- `EWDS_DID_REGISTRY_ADDRESS`
- `EWDS_MTLS_ENABLED`
- `EWDS_OFFCHAIN_STORAGE_URL`
- `EWDS_ORDERBOOK_SERVICE_URL`
- `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
- `EWDS_GATEWAY_URL` (Docker-internal API URL, defaults to `http://ddhub-gateway-api:3333`)
- `EWDS_GATEWAY_PROXY_PORT` (browser-facing proxy port, defaults to `3009`)
- `EWDS_REQUEST_FQCN` / `EWDS_RESPONSE_FQCN`
- `EWDS_ORDERS_REQUEST_TOPIC` / `EWDS_ORDERS_RESPONSE_TOPIC`
- `EWDS_TRADES_REQUEST_TOPIC` / `EWDS_TRADES_RESPONSE_TOPIC`
- `EWDS_MEASUREMENTS_REQUEST_TOPIC` / `EWDS_MEASUREMENTS_RESPONSE_TOPIC`
- `EWDS_ENABLE_HANDLER=true` (enables EWDS query responder in `gsy-orderbook-service`)

## Test Compose

Use the test compose file to run e2e and integration scenarios:

```bash
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit e2e-tests
```

EWDS-enabled test execution:

```bash
docker compose -f docker-compose.test.yml -f docker-compose.ewds.yml --profile ewds up --build --abort-on-container-exit e2e-tests
```

## Important Environment Contracts

The services expect these deployed addresses:

- `MARKET_CONTROLLER_ADDRESS`
- `ORDER_REGISTRY_ADDRESS`
- `TRADE_SETTLEMENT_ADDRESS`
- `GSY_VAULT_ADDRESS`

In default local setup, they are provisioned by the bootstrap container and injected via compose env.
