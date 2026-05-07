# EWDS Integration for GSY DEX

## Context

This document details the GSY DEX services integration with Energy Web Digital Spine (EWDS). The document describes :

1. The current off-chain service communication model.
2. The target EWDS-based communication model.
3. Required service, configuration, and Docker changes.
4. A phased rollout path that keeps local development functional.

## System Scope

In-scope services for EWDS integration:

- `gsy-orderbook-service` (off-chain storage API)
- `gsy-matching-engine`
- `gsy-execution-engine`

Related participant service:

- `gsy-community-client` (writes forecasts, measurements, market topology)

## Current Refactored Runtime

### On-chain Plane

- `anvil` (or target EVM) hosts contracts.
- `gsy-market-orchestrator` opens/closes markets.
- `gsy-community-client` publishes orders.
- `gsy-matching-engine` settles matched trades.
- `gsy-execution-engine` submits penalties.

### Off-chain Plane

- `gsy-orderbook-service` indexes chain events and exposes REST APIs.
- `gsy-matching-engine` polls `/orders`.
- `gsy-execution-engine` polls `/trades` and `/measurements`.
- `gsy-community-client` writes to `/forecasts`, `/measurements`, `/market`.

## Existing Endpoint Inventory and Callers

Provider: `gsy-orderbook-service` (`gsy-orderbook-service/src/startup.rs`)

| Endpoint | Method | Callers | Current runtime hostname |
|---|---|---|---|
| `/health_check` | `GET` | compose healthcheck, tests | `http://gsy-orderbook:8080` |
| `/orders` | `GET` | matching engine, e2e tests | `http://gsy-orderbook:8080/orders` |
| `/orders` | `POST` | e2e tests/internal tooling | `http://gsy-orderbook:8080/orders` |
| `/trades` | `GET` | execution engine, e2e tests | `http://gsy-orderbook:8080/trades` |
| `/trades` | `POST` | e2e tests/internal tooling | `http://gsy-orderbook:8080/trades` |
| `/measurements` | `GET` | execution engine | `http://gsy-orderbook:8080/measurements` |
| `/measurements` | `POST` | community client | `http://gsy-orderbook:8080/measurements` |
| `/forecasts` | `GET` | internal tooling/tests | `http://gsy-orderbook:8080/forecasts` |
| `/forecasts` | `POST` | community client | `http://gsy-orderbook:8080/forecasts` |
| `/market` | `GET` | internal tooling/tests | `http://gsy-orderbook:8080/market` |
| `/market` | `POST` | community client | `http://gsy-orderbook:8080/market` |
| `/community-market` | `GET` | community client | `http://gsy-orderbook:8080/community-market` |
| `/asset-measurements` | `GET/POST` | tests/internal | `http://gsy-orderbook:8080/asset-measurements` |

## Target EWDS Communication Model

A single Intelligent EWDS instance is used as inter-service communication backbone.

### Service Identity Model

Recommended namespaces:

- `gsy.dex.offchain-storage`
- `gsy.dex.matching-engine`
- `gsy.dex.execution-engine`
- `gsy.dex.community-client`

Each service:

1. Registers identity and credentials with EWDS.
2. Uses EWDS channels/topics for service-to-service request/response.
3. Uses schema-backed topic contracts for payload validation.

### Logical Operation Mapping

| Logical operation | Producer | Consumer | Legacy REST equivalent |
|---|---|---|---|
| `orders.query` | matching engine | off-chain storage | `GET /orders` |
| `trades.query` | execution engine | off-chain storage | `GET /trades` |
| `measurements.query` | execution engine | off-chain storage | `GET /measurements` |
| `forecasts.upsert` | community client | off-chain storage | `POST /forecasts` |
| `measurements.upsert` | community client | off-chain storage | `POST /measurements` |
| `market.upsert` | community client | off-chain storage | `POST /market` |
| `community-market.query` | community client | off-chain storage | `GET /community-market` |

## DDHub API Surface Used by Integration

The DDHub client gateway OpenAPI exposes:

- Topic management: `POST /api/v2/topics`
- Channel management: `POST /api/v2/channels`
- Messaging: `POST /api/v2/messages`, `GET /api/v2/messages`

References:

- [ddhub-client-gateway](https://github.com/energywebfoundation/ddhub-client-gateway)
- [ddhub-message-broker](https://github.com/energywebfoundation/ddhub-message-broker)
- [Energy Web Integration Guide (internal)](https://gridsingularity.atlassian.net/wiki/spaces/D3A/pages/3605823489/Energy+Web+Service+Integration)

## Schema and Validator Strategy

For each operation, define versioned request/response topic schemas:

- `gsy.dex.v1.orders.query.request`
- `gsy.dex.v1.orders.query.response`
- `gsy.dex.v1.trades.query.request`
- `gsy.dex.v1.trades.query.response`

The first concrete schema pack aligned to the Intelligent ontology CSV is now available in:

- `schemas/ewds/intelligent/`

See detailed mapping and field-level rationale in:

- `docs/platform/ewds-data-contracts.md`

Validator requirements:

- Type and required-field validation.
- Bounded `start_time`/`end_time` ranges.
- Explicit `error_code` and `error_message` payloads for failures.
- Backward-compatible schema evolution (semantic versioning).

## Service Changes Required

### gsy-orderbook-service

- EWDS query handlers are implemented for `orders.query`, `trades.query`, and `measurements.query`.
- Keep existing REST endpoints during migration for compatibility.
- Publish consistent response envelopes and error payloads.
- Runtime switch for responder path: `EWDS_ENABLE_HANDLER=true`.

### gsy-matching-engine

- Replace direct `/orders` polling path with EWDS `orders.query` request flow.
- Keep fallback transport via direct HTTP until cutover is complete.
- Runtime switch via `OFFCHAIN_STORAGE_TRANSPORT=http|ewds`.
- EWDS endpoint variables: `EWDS_GATEWAY_URL`, `EWDS_REQUEST_FQCN`, `EWDS_RESPONSE_FQCN`.

### gsy-execution-engine

- Replace direct `/trades` and `/measurements` reads with EWDS operations.
- Keep fallback transport via direct HTTP until cutover is complete.
- Runtime switch via `OFFCHAIN_STORAGE_TRANSPORT=http|ewds`.
- EWDS endpoint variables: `EWDS_GATEWAY_URL`, `EWDS_REQUEST_FQCN`, `EWDS_RESPONSE_FQCN`.

### gsy-community-client

- Route topology/forecast/measurement writes through EWDS-backed endpoint.
- Keep fallback transport via direct HTTP until cutover is complete.

## Docker and Local Testing Integration

A local DDHub Client Gateway should be deployed against EWF-hosted EWC Digital Spine services:

- Gateway-only stack: `docker-compose.ewds.yml`
- Full DEX overlay: `docker-compose.yml` plus `docker-compose.ewds.yml` with profile `ewds`

Gateway smoke-test example:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

After configuring mTLS and the DID/EWC private key through the gateway UI, restart the gateway compose stack without deleting volumes. This preserves Vault/Postgres state while forcing the API and scheduler to reload certificate and identity material:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml down --remove-orphans
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

The overlay provides:

- DDHub client gateway services.
- Vault and Postgres dependencies for local gateway setup.
- EWF mainnet EWC broker/cache/RPC configuration.
- Service-level env overrides so off-chain calls can target EWDS gateway.
