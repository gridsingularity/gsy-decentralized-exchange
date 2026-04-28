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

Out-of-scope for this phase:

- Replacing direct EVM RPC traffic (`ws://anvil:8545`) with EWDS.
- Smart-contract protocol changes.

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

A local EWDS overlay compose should be used in addition to base compose:

- Base: `docker-compose.yml`
- EWDS overlay: `docker-compose.ewds.yml`

Run example:

```bash
docker compose -f docker-compose.yml -f docker-compose.ewds.yml --profile ewds up --build
```

The overlay provides:

- DDHub client gateway services.
- Vault and Postgres dependencies for gateway bootstrap.
- Service-level env overrides so off-chain calls can target EWDS gateway.

## Phased Rollout Plan

1. **Inventory and Contract Definition**
- Finalize endpoint inventory and caller mapping.
- Define topic/channel naming and schema versions.

2. **Dual-Transport Refactor**
- Add `OFFCHAIN_STORAGE_URL` transport override to all off-chain callers.
- Keep direct HTTP as default fallback.

3. **EWDS Registration and Routing**
- Register services, topics, and channels in Intelligent EWDS.
- Deploy validators for all request/response schemas.

4. **Compose and Test Migration**
- Add EWDS compose overlay for local development.
- Add e2e scenarios that run through EWDS transport.

5. **Cutover**
- Switch service defaults to EWDS URLs.
- Remove direct HTTP fallback paths once production confidence is reached.

## Acceptance Criteria

- All in-scope services are registered in Intelligent EWDS.
- Topic/channel schemas and validators are deployed and versioned.
- Matching and execution services read required off-chain data via EWDS.
- Community client writes required off-chain data via EWDS.
- Local compose supports EWDS-enabled e2e execution.
- Existing EVM settlement and penalty flows remain functional.
