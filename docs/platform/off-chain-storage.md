# GSY DEX Off-Chain Storage

## Purpose

`gsy-offchain-storage` is the off-chain state API and persistence layer.
It stores indexed on-chain events plus ontology-aligned market and profile data.

Backend: MongoDB (`mongo:5.0`).

## Event Indexing Path

1. `gsy-ethers-listener` subscribes to:
   - `OrderPlaced`
   - `OrderCancelled`
   - `TradeSettled`
   - `NewMarketCreated`
   - `MarketClearing`
2. `OffchainStorageEvmHandler` maps event payloads into DB schemas.
3. `gsy-offchain-storage` updates order/trade records and exposes them via REST APIs.

`OrderPlaced` contains the optional order requirements (bids and offers) and
offer attributes, so the indexed order is complete without a follow-up
`/orders` write. `POST /orders` and `POST /orders-normalized` remain available as
direct database upserts, but neither is used by the E2E scenarios or production
order lifecycle. Orders are submitted through `placeOrder` (directly or through
the community client's EWDS subscriber), then indexed by the EVM listener.
The POST endpoints still accept caller-supplied statuses; their ownership and
access policy remain to be clarified. Posting trades leaves referenced orders
unchanged; settlement updates are handled by the EVM listener.
Restricting the order POST endpoints is a separate follow-up, not part of
DD-423. Until then, listener ownership is the normal lifecycle path, not an
enforced access-control boundary for those legacy write endpoints.

`NewMarketCreated` carries the full market record. The handler converts it to
`MarketSchema` (`market_id` as `0x` + 32 hex digits, `community_id` as UUID,
times as zero-padded epoch seconds) and upserts it by `market_id`, so a
replayed event is harmless. The chain is the source of markets; no service
posts them any more.

## HTTP API Surface

### Health
- `/health_check` (`GET`)

### Order Book Storage (D3.2 section 5.4)
- `/orders-normalized` (`POST`)
- `/orders` (`GET`, `POST`)
- `/flexibility-orders` (`GET`, `POST`)
- `/tariffs` (`GET`, `POST`)

### Trades Storage (D3.2 section 5.3)
- `/trades-normalized` (`POST`)
- `/trades` (`GET`, `POST`); `GET` follows the `trades.query` rules: `market_id` alone is enough, otherwise both `start_time` and `end_time` are required and span at most 1 day (`400` otherwise)
- `/market` (`GET`) compatibility adapter for EVM JSON callers
- `/communities` (`GET`, `POST`) for idempotent community query/upsert
- `/markets` (`GET`, `POST`) for ontology-aligned market-opening records;
  records come from `NewMarketCreated`, `POST` remains for manual use
- `/clearing-results` (`GET`, `POST`)
- `/market-roles` (`GET`, `POST`)

### Measurements Storage (D3.2 section 5.2)
- `/measurements` (`GET`, `POST`)
- `/forecasts` (`GET`, `POST`)
- `/measurement-points` (`GET`, `POST`) for ontology-aligned measurement metadata
- `/timeseries` (`GET`, `POST`) for ontology-aligned values

### Grid Topology and Market Storage (D3.2 section 5.1)
- `/assets` (`GET`, `POST`)
- `/pilot-sites` (`GET`, `POST`)
- `/communities` (`GET`, `POST`)
- `/sites` (`GET`, `POST`)
- `/facilities` (`GET`, `POST`)
- `/facilities/validate` (`POST`) for read-only bulk facility/market validation

### Facility Participation Validation

`POST /facilities/validate` accepts an array of
`{"facility_id": "facility-a", "market_id": "0x..."}` pairs. Facility IDs are
the original off-chain IDs, not owner IDs or generated on-chain IDs. Market IDs
accept UUID or bytes16 hex; results echo the supplied IDs.

Each pair requires an existing market and facility, the facility's site to
exist and list that facility, and the market's community to exist and list
that site. The lookup uses the latest stored topology; an ID mapping alone
does not establish participation. This does not validate market opening times
or wallet permissions.

HTTP 200 returns one `{facility_id, market_id, eligible, reason?}` per pair,
preserving order and duplicates. Ineligible pairs carry a reason:
`unknown_market`, `unknown_facility`, `unknown_community`, `unknown_site`,
`facility_not_listed_in_site` or `site_not_in_community`. Empty requests return
`[]`. Malformed requests return 400; database/deserialization failures return
500 with `code: VALIDATION_UNAVAILABLE`, not negative eligibility results.

EWDS exposes the same validation through `facilities.validate` when
`EWDS_ENABLE_HANDLER=true`. `OffchainStorageClient::validate_facility_markets`
selects HTTP or EWDS via the client's existing transport configuration and
rejects incomplete or inconsistent responses. Neither transport writes orders
or ID mappings. The community client's EWDS order subscriber validates new
orders through this API before resolving IDs and submitting them. The forecast
publication path and service-only contract authorization remain subsequent
DD-457 steps; this endpoint alone does not restrict direct contract calls.

### ID Mapping
- `/ids` (`POST`) — get-or-create offchain↔onchain ID mappings

Compatibility adapters for EVM JSON callers:

- `/measurements` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/forecasts` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/market` (`GET`) converts compatibility market JSON to/from `Market`

These adapters do not own separate collections. They read and write the same
`markets`, `measurement_points`, and `timeseries` records as the canonical API.

When `EWDS_ENABLE_HANDLER=true`, the same community collection is available
through the `communities.query` request/reply operation.
The off-chain storage then also stores the measurements, facilities, sites and
communities that other systems publish on the EWDS events channel, in
the same collections (see
[Inbound Events](ewds-integration.md#inbound-events)).

## Order Expiry

The EVM listener expires `Submitted` orders when a confirmed block reaches their
market's indexed on-chain `closingTime`. It processes that block's contract events
first, then checks expiry, including blocks with no contract events. Only open
orders in closed markets are updated, including residuals; consumed and cancelled
orders remain unchanged. `status_updated_at` is the scheduled closing time, so
late indexing and repeated blocks preserve the same deadline. Delivery-slot start
does not trigger expiry. Market windows are immutable and cannot be reopened.

The former wall-clock expiry scheduler and `scheduler_interval` setting have
been removed. HTTP and EWDS use the same listener; order-status notifications
are published only when EWDS is enabled. The first new confirmed block after startup
also checks already-indexed markets for expiry. The listener retries failed blocks
and catches up across reconnects during the same process; it has no persistent
replay cursor across restarts. Missed market/order events still require reindexing.

## Data Model Notes

- The listener creates `Matched` trades after on-chain match confirmation.
  `DeliveryVerified` advances penalty-free trades to `Executed`;
  `PenaltyRecorded` sets terminal `Penalized`, which verification does not replace.
  `Settled` means financial settlement complete. Its transition is not implemented
  until the partner's billing service is available. Trades cannot be `Rejected`.
  Records indexed under the former `Settled = on-chain confirmation` convention
  require reindexing (or a fresh validation database); they are not migrated automatically.
- Existing `POST /trades` and `/trades-normalized` endpoints retain insert-only
  import behavior. They do not update orders or publish lifecycle notifications;
  the matching engine does not use them.
- With `EWDS_ENABLE_HANDLER=true`, indexed match confirmations, penalties and
  delivery verifications publish persisted snapshots through `trade.event`.
  Replays can notify again but preserve stored statuses and timestamps.
  The event's `occurredAt` uses `status_updated_at`, falling back to creation time
  for legacy records; the trade DTO's `timestamp` remains its RFC 3339 creation time.
  HTTP and EWDS queries return the same DTO. HTTP-only operation needs no gateway.
  Notification delivery remains best effort, without a durable outbox or ordering guarantee.
- Order IDs and market IDs are stored as hex strings (`0x...`).
- The preferred trading partner of a bid or an offer is stored in its
  requirements (`requirements.trading_partner_id`) as the original off-chain
  partner facility ID; attributes carry no partner.
  Callers resolve these through `POST /ids` or EWDS `ids.query` before contract
  submission. The event indexer uses the ID mapping collection to recover the
  facility IDs; the matcher resolves them back before comparing on-chain actors.
  UUID-shaped facility IDs are mapping inputs, not already-encoded on-chain IDs.
  Missing reverse mappings cause an indexing error rather than fabricating an ID.
  The listener logs handler errors without replaying failed events, so mappings
  must exist before order submission; restoring them requires explicit reindexing.
  Existing records with on-chain partner IDs must be migrated using the mapping
  collection (or reindexed after mappings are restored); they must not be used
  as off-chain inputs to create new mappings.
- Settlement events mark consumed orders `Filled`, or `PartiallyFilled` when a
  residual ID is emitted. Each residual is a separate `Submitted` order; filling
  it later does not change its original's status. Settlement does not mark orders
  `Executed`; the later `DeliveryVerified` event does so for that trade and only
  its two consumed orders, including `PartiallyFilled` originals. Residuals are
  independent. Consumed orders use the verification block timestamp. A penalty-free
  trade uses it too; a Penalized trade keeps its first penalty block timestamp.
  Replays preserve these timestamps. Separately penalized trades remain eligible
  for verification; the execution engine skips already verified trades on-chain.
  This also makes verified trades available to the status-time/GoO query.
- Order placement, cancellation and settlement indexing use the event block's
  timestamp for `status_updated_at` (exposed as `updatedAt` in HTTP/EWDS DTOs),
  not the indexing time. Status and timestamp are updated together. Residual
  orders have independent status timestamps; `creation_time` remains unchanged.
  Existing records without a status timestamp are not backfilled by this change.
- Trade records include both order payload snapshots and selected settlement parameters.

## Operational Configuration

Key env variables:

- `EVM_NODE_URL`
- `CONTRACT_ORDER_REGISTRY`
- `CONTRACT_TRADE_SETTLEMENT`
- `CONTRACT_MARKET_CONTROLLER`
- `DATABASE_*`
- `EWDS_ENABLE_HANDLER`
- `EWDS_COMMUNITIES_REQUEST_TOPIC`
- `EWDS_EVENT_PUBLISH_FQCN`, `EWDS_EVENT_SUBSCRIBE_FQCN`
- `EWDS_MEASUREMENTS_EVENT_TOPIC`, `EWDS_FACILITY_EVENT_TOPIC`,
  `EWDS_SITE_EVENT_TOPIC`, `EWDS_COMMUNITY_EVENT_TOPIC`
