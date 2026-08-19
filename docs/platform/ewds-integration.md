# EWDS Integration for GSY DEX

## Context

This document details the GSY DEX services integration with Energy Web Digital Spine (EWDS). The document describes :

1. The current off-chain service communication model.
2. The target EWDS-based communication model.
3. Required service, configuration, and Docker changes.
4. A phased rollout path that keeps local development functional.

## System Scope

In-scope services for EWDS integration:

- `gsy-offchain-storage` (off-chain storage API)
- `gsy-market-orchestrator`
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
- `gsy-market-orchestrator` fetches communities before each scheduling tick.
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
| `/communities` | `GET/POST` | market orchestrator, pilot sites, e2e tests | `http://gsy-offchain-storage:8080/communities` |
| `/markets` | `GET/POST` | ontology-aligned market-opening API | `http://gsy-offchain-storage:8080/markets` |
| `/measurement-points` | `GET/POST` | ontology-aligned profile metadata API | `http://gsy-offchain-storage:8080/measurement-points` |
| `/timeseries` | `GET/POST` | ontology-aligned value API | `http://gsy-offchain-storage:8080/timeseries` |
| `/measurements` | `GET/POST` | EVM JSON compatibility adapter | converts to/from `MeasurementPoint` + `Timeseries` |
| `/forecasts` | `GET/POST` | EVM JSON compatibility adapter | converts to/from `MeasurementPoint` + `Timeseries` |
| `/market` | `GET/POST` | EVM JSON compatibility adapter | converts to/from `Market` |
| `/community-market` | `GET` | EVM JSON compatibility adapter | queries `Market` by community/delivery window |

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
| `community.upsert` over `communityUpsert` / `communityUpsertResponse` | pilot integration or e2e runner | off-chain storage service | off-chain storage service | request publisher | `POST /communities` |
| `communities.query` over `communitiesQuery` / `communitiesQueryResponse` | market orchestrator | off-chain storage service | off-chain storage service | market orchestrator | `GET /communities` |
| `forecasts.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `measurements.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `market.upsert` | community client | off-chain storage service | none | none | `POST /markets` |
| `community-market.query` | community client | off-chain storage service | off-chain storage service | community client | `GET /markets` |

### Local Channel Layout

| Local channel FQCN | Gateway type | Attached topics | Default env var |
|---|---|---|---|
| `gsy.intelligent.requests.pub` | Publish | `ordersQuery`, `tradesQuery`, `measurementsQuery`, `communityUpsert`, `communitiesQuery` | `EWDS_REQUEST_PUBLISH_FQCN` |
| `gsy.intelligent.requests.sub` | Subscribe | `ordersQuery`, `tradesQuery`, `measurementsQuery`, `communityUpsert`, `communitiesQuery` | `EWDS_REQUEST_SUBSCRIBE_FQCN` |
| `gsy.intelligent.responses.pub` | Publish | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse`, `communityUpsertResponse`, `communitiesQueryResponse` | `EWDS_RESPONSE_PUBLISH_FQCN` |
| `gsy.intelligent.responses.sub` | Subscribe | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse`, `communityUpsertResponse`, `communitiesQueryResponse` | `EWDS_RESPONSE_SUBSCRIBE_FQCN` |

All four channels should use the `user.roles.integration.apps.intelligent.auth.ewc` role restriction for the initial service-to-service tests.

## DDHub API Surface Used by Integration

The DDHub client gateway OpenAPI exposes:

- Topic management: `POST /api/v2/topics`
- Channel management: `POST /api/v2/channels`
- Messaging: `POST /api/v2/messages`, `GET /api/v2/messages`

References:

- [ddhub-client-gateway](https://github.com/energywebfoundation/ddhub-client-gateway)
- [ddhub-message-broker](https://github.com/energywebfoundation/ddhub-message-broker)
- [DDHub Client Gateway topics guide](https://docs.energyweb.org/energy-solutions/digital-spine-by-energy-web/component-guides/ddhub-client-gateway/technical-guide/topics)
- [DDHub Client Gateway channels guide](https://docs.energyweb.org/energy-solutions/digital-spine-by-energy-web/component-guides/ddhub-client-gateway/technical-guide/channels)
- [Energy Web Integration Guide (internal)](https://gridsingularity.atlassian.net/wiki/spaces/D3A/pages/3605823489/Energy+Web+Service+Integration)

## EWF Runtime Constraints

EWF confirmed these broker/runtime limits for the shared Intelligent EWDS setup:

- Basic messaging payload limit: 6 MB including metadata.
- File transfer payload limit: 100 MB.
- Message retention: 24 hours, then physically removed from broker storage.
- Payload encryption can be enabled, but it reduces effective message size and adds performance cost.

## Schema and Validator Strategy

For each operation, define versioned request/response topic schemas. DDHub topic names use camelCase because the gateway UI rejects dots in topic names; the payload `operation` field keeps the dotted operation name for service routing.

- `ordersQuery` (`operation=orders.query`)
- `ordersQueryResponse`
- `tradesQuery` (`operation=trades.query`)
- `tradesQueryResponse`
- `measurementsQuery` (`operation=measurements.query`)
- `measurementsQueryResponse`
- `communityUpsert` (`operation=community.upsert`)
- `communityUpsertResponse`
- `communitiesQuery` (`operation=communities.query`)
- `communitiesQueryResponse`
- `forecastsQuery`
- `forecastsQueryResponse`
- `openMarketsQuery`
- `openMarketsQueryResponse`
- `topologyQuery`
- `topologyQueryResponse`

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

### primitives

- `EwdsClientConfig` resolves gateway, FQCN, topic, client-ID, and polling settings from the environment once when a client is created.
- `EwdsOperation` maps each query operation to its configured request/response topic pair; callers pass only the operation and query payload.
- `EwdsClient` separates request publishing from response polling behind its `query` method.
- EWDS wire DTOs and database-schema conversions are isolated in `ewds::dto`.

### gsy-offchain-storage

- EWDS handlers are implemented for `orders.query`, `trades.query`,
  `measurements.query`, `community.upsert`, and `communities.query`.
- Order payloads are emitted with Intelligent-style camelCase fields; the matching-engine consumer still accepts legacy native `DbOrderSchema` payloads during migration.
- Keep existing REST endpoints during migration for compatibility.
- Publish consistent response envelopes and error payloads.
- Runtime switch for responder path: `EWDS_ENABLE_HANDLER=true`.

### gsy-matching-engine

- Replace direct `/orders` polling path with EWDS `orders.query` request/reply over the local client gateway.
- Keep fallback transport via direct HTTP until cutover is complete.
- Runtime switch via `OFFCHAIN_STORAGE_TRANSPORT=http|ewds`.
- EWDS endpoint variables: `EWDS_GATEWAY_URL`, `EWDS_REQUEST_PUBLISH_FQCN`, `EWDS_RESPONSE_SUBSCRIBE_FQCN`, `EWDS_TOPIC_OWNER`, `EWDS_TOPIC_VERSION`, `EWDS_MATCHING_ENGINE_CLIENT_ID`.
- Confirmed runtime defaults: `EWDS_REQUEST_PUBLISH_FQCN=gsy.intelligent.requests.pub`, `EWDS_RESPONSE_SUBSCRIBE_FQCN=gsy.intelligent.responses.sub`, `EWDS_TOPIC_OWNER=integration.apps.intelligent.auth.ewc`.

### gsy-execution-engine

- Replace direct HTTP reads for `/trades`, `/measurement-points`, and `/timeseries` with EWDS operations.
- Keep fallback transport via direct HTTP until cutover is complete.
- Runtime switch via `OFFCHAIN_STORAGE_TRANSPORT=http|ewds`.
- EWDS endpoint variables: `EWDS_GATEWAY_URL`, `EWDS_REQUEST_PUBLISH_FQCN`, `EWDS_RESPONSE_SUBSCRIBE_FQCN`, `EWDS_TOPIC_OWNER`, `EWDS_TOPIC_VERSION`, `EWDS_EXECUTION_ENGINE_CLIENT_ID`.
- Confirmed runtime defaults: `EWDS_REQUEST_PUBLISH_FQCN=gsy.intelligent.requests.pub`, `EWDS_RESPONSE_SUBSCRIBE_FQCN=gsy.intelligent.responses.sub`, `EWDS_TOPIC_OWNER=integration.apps.intelligent.auth.ewc`.

### gsy-community-client

- Route facility-topology-derived market, forecast, and measurement writes through ontology-aligned off-chain storage APIs.
- Keep fallback transport via direct HTTP until cutover is complete.

### gsy-market-orchestrator

- Fetch all communities before every scheduling tick through HTTP
  `/communities` or EWDS `communities.query`.
- Derive each market ID from community UUID, market type, and delivery slot.
- Open and close the community/market-type permutations through batched contract
  calls.
- Runtime switch via `OFFCHAIN_STORAGE_TRANSPORT=http|ewds`.

## Docker and Local Testing Integration

A local DDHub Client Gateway should be deployed against EWF-hosted EWC Digital Spine services:

- Gateway-only stack: `docker-compose.ewds.yml`
- GSY DEX EWDS mode: `docker-compose.yml` or `docker-compose.test.yml` with `.env.ewds.local`
- Gateway namespace validator: `APPLICATION_NAMESPACE_REGULAR_EXPRESSION=\w+\.apps\..*\.(iam|auth)\.ewc` for both API and scheduler, as required by EWF for Intelligent `.auth.ewc` application namespaces.

Operational startup order:

1. Start the local DDHub Client Gateway stack.
2. Confirm the gateway dashboard is online, mTLS is valid, IAM login succeeds, and scheduler jobs report success.
3. In the Client Gateway UI, create or verify the four request/response publish/subscribe channels.
4. Attach the required Intelligent-owned request/response topics to the matching channels.
5. Start the GSY services from the normal compose file with `.env.ewds.local`.

Channel/topic setup notes:

- Topic application/owner: `integration.apps.intelligent.auth.ewc`.
- Local channel FQCNs: `gsy.intelligent.requests.pub`, `gsy.intelligent.requests.sub`, `gsy.intelligent.responses.pub`, `gsy.intelligent.responses.sub`.
- Required topics: `ordersQuery`, `ordersQueryResponse`, `tradesQuery`,
  `tradesQueryResponse`, `measurementsQuery`, `measurementsQueryResponse`,
  `communityUpsert`, `communityUpsertResponse`, `communitiesQuery`, and
  `communitiesQueryResponse`.
- Topic creation requires `topiccreator`; channel creation requires gateway admin access.
- The gateway API validates send requests against a `pub` channel and receive polling against a `sub` channel. The direction-specific FQCN env vars are the default integration path.
- Gateway smoke testing confirmed that message payloads must be JSON-encoded strings, sends must include `topicVersion`, `transactionId`, and `anonymousRecipient`, and receive polling must use `GET /api/v2/messages` with an alphanumeric `clientId` cursor.

Validated e2e status:

- The full Cucumber e2e suite passed with EWDS mode enabled: `2` features, `2` scenarios, and `20` steps passed.
- The validated test command is documented in `docs/setup/test.md`.
- DDHub delivery is asynchronous; use `EWDS_RESPONSE_TIMEOUT_MS=60000` for
  deterministic e2e runs. The GSY clients and responder apply exponential
  backoff to direct or Client-Gateway-wrapped `429` responses; tune it with
  `EWDS_RATE_LIMIT_BACKOFF_MS` and `EWDS_RATE_LIMIT_MAX_BACKOFF_MS`.

Gateway smoke-test example:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

After configuring mTLS and the DID/EWC private key through the gateway UI, restart the gateway compose stack without deleting volumes. This preserves Vault/Postgres state while forcing the API and scheduler to reload certificate and identity material:

```bash
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml down --remove-orphans
docker compose --env-file .env.ewds.local -f docker-compose.ewds.yml up --build
```

The gateway compose provides:

- DDHub client gateway services.
- Vault and Postgres dependencies for local gateway setup.
- EWF mainnet EWC broker/cache/RPC configuration.

The contracts compose file provides the local Anvil chain and contract
deployment. The normal GSY DEX compose files provide MongoDB and GSY services.
They read `contracts-output/addresses.env` for contract addresses and
`.env.ewds.local` to switch service communication from direct HTTP to the local
DDHub Client Gateway.
