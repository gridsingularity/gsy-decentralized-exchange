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
| `clearing_results.query` over `clearingResultsQuery` / `clearingResultsQueryResponse` | matching/execution engine | off-chain storage service | off-chain storage service | requester | `GET /clearing-results` |
| `markets.query` over `marketsQuery` / `marketsQueryResponse` | community client | off-chain storage service | off-chain storage service | community client | `GET /markets` |
| `ids.query` over `idsQuery` / `idsQueryResponse` | requester service | off-chain storage service | off-chain storage service | requester | `POST /ids` (get-or-create) |
| `community.submitted` event over `community` | other systems or e2e runner | off-chain storage service | none | none | `POST /communities` |
| `order.submitted` event over `order` | FOS or e2e runner | community client | none | none | none (the community client calls `OrderRegistry.placeOrder`) |
| `communities.query` over `communitiesQuery` / `communitiesQueryResponse` | market orchestrator | off-chain storage service | off-chain storage service | market orchestrator | `GET /communities` |
| `forecasts.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `measurements.upsert` | community client | off-chain storage service | none | none | `POST /measurement-points` + `POST /timeseries` |
| `market.upsert` | community client | off-chain storage service | none | none | `POST /markets` |

> Note: the `*.query` operations above are implemented in the current responder
> (`EwdsOperation` variants `OrdersQuery`, `TradesQuery`, `MeasurementsQuery`,
> `ClearingResultsQuery`, `MarketsQuery`, `IdsQuery`). The `*.upsert` operations
> for forecasts, measurements and markets remain future work; those writes still
> go over the REST compatibility path. Communities are written over EWDS with
> the `community.submitted` event (see [Inbound Events](#inbound-events)).

| Local channel FQCN | Gateway type | Attached topics | Default env var |
|---|---|---|---|
| `gsy.intelligent.requests.pub` | Publish | `ordersQuery`, `tradesQuery`, `measurementsQuery`, `clearingResultsQuery`, `marketsQuery`, `idsQuery`, `communitiesQuery` | `EWDS_REQUEST_PUBLISH_FQCN` |
| `gsy.intelligent.requests.sub` | Subscribe | `ordersQuery`, `tradesQuery`, `measurementsQuery`, `clearingResultsQuery`, `marketsQuery`, `idsQuery`, `communitiesQuery` | `EWDS_REQUEST_SUBSCRIBE_FQCN` |
| `gsy.intelligent.responses.pub` | Publish | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse`, `clearingResultsQueryResponse`, `marketsQueryResponse`, `idsQueryResponse`, `communitiesQueryResponse` | `EWDS_RESPONSE_PUBLISH_FQCN` |
| `gsy.intelligent.responses.sub` | Subscribe | `ordersQueryResponse`, `tradesQueryResponse`, `measurementsQueryResponse`, `clearingResultsQueryResponse`, `marketsQueryResponse`, `idsQueryResponse`, `communitiesQueryResponse` | `EWDS_RESPONSE_SUBSCRIBE_FQCN` |
| `gsy.intelligent.events.pub` | Publish | `trade`, `clearingResult`, `market`, `measurements`, `facility`, `site`, `community`, `order` | `EWDS_EVENT_PUBLISH_FQCN` |
| `gsy.intelligent.events.sub` | Subscribe | `trade`, `clearingResult`, `market`, `measurements`, `facility`, `site`, `community`, `order` | `EWDS_EVENT_SUBSCRIBE_FQCN` |

The events channels carry fire-and-forget domain events rather than
request/reply traffic. When `EWDS_ENABLE_HANDLER` is on, the off-chain storage
publishes a `trade.created` event on `trade` every time it persists a
trade from an EVM `TradeSettled` event, a `clearing_result.created` event on
`clearingResult` every time it persists a clearing result from an EVM
`MarketClearing` event, and a `market_status.updated` event on
`market` for every EVM `MarketStatusUpdated` event. The payload is
an event envelope (`eventId`, `eventType`, `occurredAt`, `data`) whose `data` is
a list of trades, clearing results or market statuses in their `EwdsTradeDto`,
`EwdsClearingResultDto` or `EwdsMarketStatusDto` (`marketId`, `isOpen`) form,
so one event can carry several of them. The off-chain storage currently sends
one item per EVM event. Every event gets a random UUID as `eventId`, which is
also its DDHub `transactionId`. `occurredAt` is an RFC 3339 timestamp: the time
of the newest item. Market status updates are not persisted, so their
`occurredAt` is the time the off-chain storage received the EVM event.

The same channels also carry the events other systems send to GSY; see
[Inbound Events](#inbound-events).

The broad `user.roles.integration.apps.intelligent.auth.ewc` restriction can be
used for an initial delivery smoke test. For request/reply operation, the
request publish channel must resolve only to the authoritative GSY
off-chain-storage responder DID. If multiple qualified responders consume the
same request topics, they can return different snapshots for the same request
ID. The response publish channel can resolve to all GSY request clients.

### Polling

Every service polls each subscribe channel it reads as a whole. The
`GET /api/v2/messages` call sends only `fqcn`, `amount` and `clientId`, with
no `topicName` and no `topicOwner`, so the gateway returns the messages of
every topic on the channel. The service then:

- drops messages whose `topicOwner` is not `EWDS_TOPIC_OWNER`,
- hands every other message to the queue of its `topicName`, in arrival order,
- drops the messages of topics it doesn't consume. These are expected, because
  a service also receives the topics meant for other services.

Each message the gateway returns carries `topicName`, `topicOwner`,
`topicVersion`, `id`, `transactionId` and `sender` next to `payload`. If a
message ever has no `topicName`, it is routed by its payload instead: an
event by `eventType`, a request by `operation`.

| Channel | Polled by | `clientId` |
|---|---|---|
| `gsy.intelligent.requests.sub` | off-chain storage | `EWDS_REQUEST_CLIENT_ID` + `requests`, e.g. `gsyoffchainstoragerequests` |
| `gsy.intelligent.events.sub` | off-chain storage | `EWDS_REQUEST_CLIENT_ID` + `events`, e.g. `gsyoffchainstorageevents` |
| `gsy.intelligent.events.sub` | community client | `EWDS_COMMUNITY_CLIENT_ID` + `events`, e.g. `gsycommunityclientevents` |
| `gsy.intelligent.responses.sub` | every query caller | client ID + response topic, e.g. `gsymatchingengineordersQueryResponse` |

Query responses are the exception: they are still polled per response topic.
Every topic has its own `clientId` cursor and the caller matches responses by
`requestId`, so this works whether or not the gateway filters by topic.

Gateway behaviour to keep in mind:

- A `clientId` is a cursor of its own. Two pollers must never share one, or
  they split the messages between them.
- A new `clientId` receives everything the broker still keeps for the channel,
  up to 24 hours.
- The first poll of a new `clientId` can take more than 30 seconds while the
  gateway sets up the consumer.
- A poll returns at most `amount` messages over all topics of the channel.
  When it returns a full batch, the service polls again at once instead of
  waiting for the next interval.

### Inbound Events

Other systems publish new or changed measurements, facilities, sites
and communities on `gsy.intelligent.events.pub`. When `EWDS_ENABLE_HANDLER` is
on, the off-chain storage takes these four topics from its poll of
`gsy.intelligent.events.sub` (see [Polling](#polling)) and stores the data in the same collections the
REST API uses. New orders go to the community client instead; see
[Order Events](#order-events).

| Topic | `eventType` | `data` | Stored with |
|---|---|---|---|
| `measurements` | `measurements.submitted` | array of 1..N measurements (`EwdsMeasurementDto`) | measurement points and timeseries, upserted by point and timestamp |
| `facility` | `facility.submitted` | array of 1..N facilities (`FacilitySchema`) | upserted by `facility_id` |
| `site` | `site.submitted` | array of 1..N sites (`SiteSchema`) | upserted by `site_name` |
| `community` | `community.submitted` | array of 1..N communities (`EwdsCommunityDto`) | upserted by `communityId` |

The envelope is the one GSY uses for its own events:

```json
{
  "eventId": "7d3f7a52-0c5e-4a8e-9a57-2b8f5f3f7e10",
  "eventType": "measurements.submitted",
  "occurredAt": "2026-09-29T10:16:02Z",
  "data": [
    {
      "facilityId": "facility-1",
      "communityUuid": "community-1",
      "timeSlot": "2026-09-29T10:00:00Z",
      "creationTime": "2026-09-29T10:15:30Z",
      "energyKwh": 1.25
    }
  ]
}
```

- Every event carries a list, so one event can submit many items. Its size is
  only limited by the 6 MB message limit.
- Measurements use camelCase fields with RFC 3339 times. `timeSlot` is the
  start of the 15-minute slot, and `energyKwh` is positive for consumed and
  negative for produced energy. A batch can mix facilities and slots.
- Facilities and sites use the snake_case fields of `FacilitySchema`
  (`facility_id`, `facility_name`, `site_id`, `owner_id`) and `SiteSchema`
  (`site_name`, `site_description`, `facilities`), the same form
  `facilities.query` returns. A facility's `site_id` refers to a `site_name`.
- Communities use the camelCase fields of `EwdsCommunityDto` (`communityId`,
  `communityName`, `sites`), the same form `communities.query` returns.

Handling rules:

- `eventId` identifies an event. A re-sent event must keep its `eventId`; the
  off-chain storage drops IDs it has already handled. Because all writes are
  upserts, an event that arrives again only updates its record.
- Events whose `eventType` doesn't match their topic, malformed messages and
  invalid data are logged and skipped, and the next message is still handled.
  One invalid item rejects the whole event, so nothing of it is stored; a
  corrected event needs a new `eventId`.
- A failed database write is tried up to three times in a row. A duplicate
  key, such as a facility or community name that is already taken, counts as
  invalid data and is not retried.
- The gateway acknowledges the messages of a poll at the next poll of the same
  `clientId`, so it never delivers a failed event again. The subscriber therefore keeps a failed event and
  retries it, up to `EWDS_EVENT_HANDLE_ATTEMPTS` (default 8) attempts in
  total. The first retry waits `EWDS_EVENT_RETRY_DELAY_MS` (default 2 000 ms),
  and the delay doubles with every attempt up to 5 minutes, so the defaults
  cover an outage of about 4 minutes. Then the event is logged and dropped.
- Events of one topic are handled in the order they arrive. While a failed
  event waits for its retry, the later events of its topic wait too, so an
  older event never overwrites the data of a newer one. The other topics carry
  on. Failed events are only
  kept in memory and are lost if the service restarts.
- The broker keeps messages for 24 hours, so events sent while the off-chain
  storage is down for longer are lost.

The event schemas are `int.<eventType>.event.v1.json` in
`schemas/ewds/intelligent/`, e.g. `int.facility.submitted.event.v1.json`. The
e2e stack uses the `...Test` variants of the inbound topics
(`measurementsTest`, ..., `orderTest`).

#### Order Events

FOS publishes new orders on the `order` topic
(`EWDS_ORDER_EVENT_TOPIC`). When `EWDS_ENABLE_HANDLER` is on, the
community client handles only this topic of `gsy.intelligent.events.sub`. It
polls the channel with the client ID `EWDS_COMMUNITY_CLIENT_ID` + `events` and
drops the other topics (see [Polling](#polling)). It sends each order to
`OrderRegistry.placeOrder`. The off-chain storage doesn't subscribe to it.
From there the usual flow takes over: the off-chain storage indexes
`OrderPlaced`, the matching engine matches, and trades go out as
`trade.created` events.

`data` is a list of orders in the `EwdsOrderDto` form that `orders.query`
returns (`int.order.submitted.event.v1.json`):

```json
{
  "eventId": "5e0f1c2d-3b4a-4f6e-8d7c-9a0b1c2d3e4f",
  "eventType": "order.submitted",
  "occurredAt": "2026-09-30T09:40:13Z",
  "data": [
    {
      "orderId": "3f2c6d1e-8a4b-4c7d-9e2f-1a5b6c7d8e9f",
      "marketId": "0x5b0f3c2a9d8e7f6a5b4c3d2e1f0a9b8c",
      "orderType": "bid",
      "orderStatus": "submitted",
      "timeSlot": "2026-09-30T10:00:00Z",
      "quantity": 1.5,
      "priceLimit": 0.3,
      "createdBy": "owner-1",
      "creationTime": "2026-09-30T09:40:12Z",
      "energySourcePreference": "GREEN",
      "preferredTradingPartner": "owner-2",
      "preferredEnergyRate": 0.25
    }
  ]
}
```

| Field | Rule |
|---|---|
| `orderId` | A UUID chosen by the publisher. Its 16 bytes become the on-chain order ID, so `orders.query` and the trades' `bidId`/`offerId` return it as `0x` plus 32 hex digits. |
| `marketId` | The market's hex ID, e.g. from `markets.query`. The market must be open. |
| `createdBy` | The off-chain actor ID, e.g. the facility owner. The ID service maps it to the on-chain actor ID. |
| `orderStatus` | Must be `submitted`. |
| `quantity` | Greater than 0, in kWh. |
| `priceLimit` | 0 or more, in EUR/kWh. |
| `preferredTradingPartner` | The partner's off-chain ID, resolved like `createdBy`. A requirement for bids and an attribute for offers. |
| `updatedAt`, `rejectReason` | Ignored. |

Handling rules:

- One invalid order rejects the whole event before anything is sent: parsing,
  the rules above and resolving the IDs all have to pass.
- The orders are then sent one at a time. The community client doesn't wait
  for the transactions to be mined:
  - An order whose ID is already on-chain (`getStatus`) is skipped.
  - An order the contract would reject (`MarketClosed`, `Unauthorized`,
    `InvalidOrderParams`, `OrderAlreadyExists`) is caught when the node
    estimates gas. It is logged with `eventId`, `orderId` and the reason and
    not sent, and the remaining orders are still sent.
  - A revert that only happens in the mined transaction, e.g. because the
    market closed in between, is not noticed.
  - Transport errors are tried up to three times in a row. If an order still
    can't be sent, the event fails after the other orders were sent, and the
    community client retries it like the off-chain storage retries its events
    (see [Inbound Events](#inbound-events)). Orders already on-chain are
    skipped then.
- An ID the ID service doesn't know may be registered later, so an event that
  fails on it is retried as well; invalid order data is not.
- Resending an event, even under a new `eventId`, doesn't place an order
  twice. A resend while the first transaction is still pending sends the
  order again, but that duplicate reverts with `OrderAlreadyExists`.
- FOS gets no feedback on rejected orders. It sees placed orders through
  `orders.query` and their trades through `trade.created`.
- `placeOrder` only accepts an order if the community client's wallet is
  authorized for the actor in the `ActorRegistry`. Registering it is up to
  whoever publishes the orders.

### Query Payload Fields

Each `*.query` request payload accepts both snake_case and camelCase keys (via serde aliases):

| Operation | Fields (all optional unless noted) |
|---|---|
| `orders.query` | `market_id`/`marketId`, `start_time`/`startTime`, `end_time`/`endTime` |
| `trades.query` | `market_id`/`marketId`, `start_time`/`startTime`, `end_time`/`endTime` (`market_id` takes precedence over the time range, which matches the market `delivery_start_time`) |
| `measurements.query` | `start_time`/`startTime`, `end_time`/`endTime`, `facility_id`/`areaUuid` (filters after fetch) |
| `ids.query` | `offchain_id`/`offchainId` (required) |

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
- `ewds::channel::EwdsChannelPoller` polls one subscribe channel without a
  topic filter and routes its messages to a queue per topic (see
  [Polling](#polling)).
- `EwdsClient::run_event_subscriber` polls `EWDS_EVENT_SUBSCRIBE_FQCN` (up to
  `EWDS_EVENT_BATCH_SIZE` messages) every `EWDS_EVENT_POLL_INTERVAL_MS`
  (default 1 000 ms). It passes every new event of
  the given event types to a handler, in arrival order per topic. A failed
  event is retried (`EWDS_EVENT_HANDLE_ATTEMPTS`, `EWDS_EVENT_RETRY_DELAY_MS`)
  unless the handler marks it as invalid. The off-chain storage and the
  community client share it.
- EWDS wire DTOs and database-schema conversions are isolated in `ewds::dto`.

### gsy-offchain-storage

- EWDS handlers are implemented for `orders.query`, `trades.query`,
  `measurements.query`, and `communities.query`.
- One poller reads the request channel and every operation has its own worker,
  so response retries for one operation do not block request handling for the
  others. A request whose `operation` doesn't belong to its topic is answered
  with an `OPERATION_TOPIC_MISMATCH` error. A request that fails is logged and
  doesn't hold up the rest of its poll.
- A request published longer ago than `EWDS_REQUEST_MAX_AGE_MS` (default:
  `EWDS_RESPONSE_TIMEOUT_MS`) is skipped, using the gateway's `timestampNanos`.
  Its sender has stopped waiting. Answering would only flood the response
  topic, which every requester reads, so that the fresh responses arrive
  minutes late. This matters after an outage or a new `clientId`, when a
  backlog of up to 24 hours arrives at once.
- An event subscriber stores the measurements, facilities, sites and
  communities that other systems publish on the events channel (see
  [Inbound Events](#inbound-events)).
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
- Send the orders of `order.submitted` events to `OrderRegistry.placeOrder`
  (see [Order Events](#order-events)). Runtime switch:
  `EWDS_ENABLE_HANDLER=true`.

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
- GSY DEX EWDS mode: `docker-compose.yml` or `docker-compose.e2e-test.yml` with `.env.ewds.local`
- Gateway namespace validator: `APPLICATION_NAMESPACE_REGULAR_EXPRESSION=\w+\.apps\..*\.(iam|auth)\.ewc` for both API and scheduler, as required by EWF for Intelligent `.auth.ewc` application namespaces.

Operational startup order:

1. Start the local DDHub Client Gateway stack.
2. Confirm the gateway dashboard is online, mTLS is valid, IAM login succeeds, and scheduler jobs report success.
3. In the Client Gateway UI, create or verify the four request/response publish/subscribe channels.
4. Attach the required Intelligent-owned request/response topics to the matching channels.
5. Start the GSY services from the normal compose file with `.env.ewds.local`.

Channel/topic setup notes:

- Topic application/owner: `integration.apps.intelligent.auth.ewc`.
- Local channel FQCNs: `gsy.intelligent.requests.pub`, `gsy.intelligent.requests.sub`, `gsy.intelligent.responses.pub`, `gsy.intelligent.responses.sub`, `gsy.intelligent.events.pub`, `gsy.intelligent.events.sub`.
- The events channels need the eight event topics from the channel table, plus
  `measurementsTest`, `facilityTest`, `siteTest`,
  `communityTest` and `orderTest` for e2e runs. `scripts/ewds_channel_topic_handler.sh`
  creates all topics and attaches them to the channels.
- Required topics: `ordersQuery`, `ordersQueryResponse`, `tradesQuery`,
  `tradesQueryResponse`, `measurementsQuery`, `measurementsQueryResponse`,
  `communitiesQuery`, and
  `communitiesQueryResponse`.
- Topic creation requires `topiccreator`; channel creation requires gateway admin access.
- The gateway API validates send requests against a `pub` channel and receive polling against a `sub` channel. The direction-specific FQCN env vars are the default integration path.
- Gateway smoke testing confirmed that message payloads must be JSON-encoded strings, sends must include `topicVersion`, `transactionId`, and `anonymousRecipient`, and receive polling must use `GET /api/v2/messages` with an alphanumeric `clientId` cursor. Leaving out `topicName` and `topicOwner` on the `GET` returns the messages of all topics of the channel (see [Polling](#polling)).

Validated e2e status:

- The pay-as-bid Cucumber e2e suite passed with EWDS mode enabled: `3`
  features, `3` scenarios, and `30` steps passed, including isolated trading
  and penalty settlement across two community markets.
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
