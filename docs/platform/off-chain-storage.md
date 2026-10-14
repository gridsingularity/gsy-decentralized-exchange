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
   - `MarketStatusUpdated`
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
- `/markets` (`GET`, `POST`) for ontology-aligned market-opening records
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

The EVM listener expires `Submitted` orders when it receives a confirmed
`MarketStatusUpdated(marketId, false)` event. Only that market's orders are
updated, using the closure block timestamp; the delivery slot does not trigger
expiry. This includes open residuals, but leaves consumed or cancelled orders
unchanged. Repeated closure events preserve the original expiry timestamp, and
reopening a market does not reactivate expired orders.

The former wall-clock expiry scheduler and `scheduler_interval` setting have
been removed. This uses the same listener in HTTP and EWDS mode; market event
notifications are still published only when the EWDS handler is enabled.
The listener currently has no historical replay: missed closure events require
reconciliation/reindexing, as do other missed lifecycle events.

## Data Model Notes

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
  `Executed`; delivery-verification transitions are still pending in DD-423.
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
- `SCHEDULER_INTERVAL`
- `EWDS_ENABLE_HANDLER`
- `EWDS_COMMUNITIES_REQUEST_TOPIC`
- `EWDS_EVENT_PUBLISH_FQCN`, `EWDS_EVENT_SUBSCRIBE_FQCN`
- `EWDS_MEASUREMENTS_EVENT_TOPIC`, `EWDS_FACILITY_EVENT_TOPIC`,
  `EWDS_SITE_EVENT_TOPIC`, `EWDS_COMMUNITY_EVENT_TOPIC`
