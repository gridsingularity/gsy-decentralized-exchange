# EWDS Integration for GSY DEX

## Context

This document details the GSY DEX services integration with Energy Web Digital Spine (EWDS). The document describes:

1. The current off-chain service communication model.
2. The target EWDS-based communication model.
3. Required service, configuration, and Docker changes.
4. A phased rollout path that keeps local development functional.

## System Scope

In-scope services for EWDS integration:

- `gsy-offchain-storage` (off-chain storage API)
- `gsy-matching-engine`
- `gsy-execution-engine`

Related participant service:

- `gsy-community-client` (writes forecasts, measurements, and market records)

## Current Refactored Runtime

### On-chain Plane

- `anvil` (or target EVM) hosts contracts.
- `gsy-market-orchestrator` opens/closes markets.
- `gsy-community-client` publishes orders.
- `gsy-matching-engine` settles matched trades.
- `gsy-execution-engine` submits penalties.

### Off-chain Plane

- `gsy-offchain-storage` indexes chain events and exposes REST APIs.
- `gsy-matching-engine` polls `/orders`.
- `gsy-execution-engine` polls `/trades`, `/measurement-points`, and `/timeseries`.
- `gsy-community-client` writes to `/measurement-points`, `/timeseries`, and `/markets`.

## Existing Endpoint Inventory and Callers

Provider: `gsy-offchain-storage` (`gsy-offchain-storage/src/startup.rs`)

| Endpoint | Method | Callers | Current runtime hostname |
|---|---|---|---|
| `/health_check` | `GET` | compose healthcheck, tests | `http://gsy-offchain-storage:8080` |
| `/orders` | `GET` | matching engine, e2e tests | `http://gsy-offchain-storage:8080/orders` |
| `/orders` | `POST` | e2e tests/internal tooling | `http://gsy-offchain-storage:8080/orders` |
| `/trades` | `GET` | execution engine, e2e tests | `http://gsy-offchain-storage:8080/trades` |
| `/trades` | `POST` | e2e tests/internal tooling | `http://gsy-offchain-storage:8080/trades` |
| `/markets` | `GET/POST` | ontology-aligned market-opening API | `http://gsy-offchain-storage:8080/markets` |
| `/clearing-results` | `GET/POST` | clearing-result API | `http://gsy-offchain-storage:8080/clearing-results` |
| `/ids` | `POST` | offchain↔onchain ID mapping API | `http://gsy-offchain-storage:8080/ids` |
| `/measurement-points` | `GET/POST` | ontology-aligned profile metadata API | `http://gsy-offchain-storage:8080/measurement-points` |
| `/timeseries` | `GET/POST` | ontology-aligned value API | `http://gsy-offchain-storage:8080/timeseries` |
| `/measurements` | `GET/POST` | EVM JSON compatibility adapter | converts to/from `MeasurementPoint` + `Timeseries` |
| `/forecasts` | `GET/POST` | EVM JSON compatibility adapter | converts to/from `MeasurementPoint` + `Timeseries` |
| `/market` | `GET` | EVM JSON compatibility adapter | converts to/from `Market` |

## Target EWDS Communication Model

A single Intelligent EWDS instance is used as inter-service communication backbone.

### Service Identity Model

EWF-confirmed namespace/channel model:

- Topic owner namespace: `integration.apps.intelligent.auth.ewc`
- Local Client Gateway channels: managed by us in our gateway, with separate publish/subscribe FQCNs because internal channel names must be unique
- Topic layout: multiple request/response topics can be associated with the same channel

Each service:

1. Registers identity and credentials with EWDS.
2. Uses the local Client Gateway channel and Intelligent-owned topics for service-to-service request/response.
3. Uses schema-backed topic contracts for payload validation.

### Logical Operation Mapping

| Payload operation / DDHub topics | Request publisher | Request consumer | Response publisher | Response consumer | Legacy REST equivalent |
|---|---|---|---|---|---|
| `orders.query` over `ordersQuery` / `ordersQueryResponse` | matching engine | off-chain storage service | off-chain storage service | matching engine | `GET /orders` |
| `trades.query` over `tradesQuery` / `tradesQueryResponse` | execution engine | off-chain storage service | off-chain storage service | execution engine | `GET /trades` |
| `measurements.query` over `measurementsQuery` / `measurementsQueryResponse` | execution engine | off-chain storage service | off-chain storage service | execution engine | `GET /measurement-points` + `GET /timeseries` |
| `clearing_results.query` over `clearing_resultsQuery` / `clearing_resultsQueryResponse` | matching/execution engine | off-chain storage service | off-chain storage service | requester | `GET /clearing-results` |
| `markets.query` over `marketsQuery` / `marketsQueryResponse` | community client | off-chain storage service | off-chain storage service | community client | `GET /markets` |
| `ids.query` over `idsQuery` / `idsQueryResponse` | requester service | off-chain storage service | off-chain storage service | requester | `POST /ids` (get-or-create) |
| `forecasts.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `measurements.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `market.upsert` | community client | off-chain storage service | none | none | `POST /markets` |

> Note: the `*.query` operations above are implemented in the current responder
> (`EwdsOperation` variants `OrdersQuery`, `TradesQuery`, `MeasurementsQuery`,
> `ClearingResultsQuery`, `MarketsQuery`, `IdsQuery`). The `*.upsert` operations
> remain future work — writes still go over the REST compatibility path.

### Query Payload Fields

Each `*.query` request payload accepts both snake_case and camelCase keys (via serde aliases):

| Operation | Fields (all optional unless noted) |
|---|---|
| `orders.query` | `market_id`/`marketId`, `start_time`/`startTime`, `end_time`/`endTime` |
| `trades.query` | `start_time`/`startTime`, `end_time`/`endTime`, `facility_id`/`areaUuid` |
| `measurements.query` | `start_time`/`startTime`, `end_time`/`endTime`, `facility_id`/`areaUuid` (filters after fetch) |
|