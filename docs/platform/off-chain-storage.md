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
- `/trades` (`GET`, `POST`)
- `/market` (`GET`) compatibility adapter for EVM JSON callers
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

## Scheduler Behavior

`expire_orders_scheduler` periodically marks stale open orders as `Expired` using `time_slot` and current time.

## Data Model Notes

- Order IDs and market IDs are stored as hex strings (`0x...`).
- Settlement events transition order statuses to `Executed`.
- Trade records include both order payload snapshots and selected settlement parameters.

## Operational Configuration

Key env variables:

- `EVM_NODE_URL`
- `CONTRACT_ORDER_REGISTRY`
- `CONTRACT_TRADE_SETTLEMENT`
- `CONTRACT_MARKET_CONTROLLER`
- `DATABASE_*`
- `SCHEDULER_INTERVAL`