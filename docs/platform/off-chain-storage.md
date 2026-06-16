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

- `/health_check`
- `/orders` (`GET`, `POST`)
- `/trades` (`GET`, `POST`)
- `/markets` (`GET`, `POST`) for ontology-aligned market-opening records
- `/measurement-points` (`GET`, `POST`) for ontology-aligned measurement metadata
- `/timeseries` (`GET`, `POST`) for ontology-aligned values

Compatibility adapters for EVM JSON callers:

- `/measurements` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/forecasts` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/market` (`GET`, `POST`) converts market topology JSON to/from `Market`
- `/community-market` (`GET`) queries ontology market records by community and delivery window

These adapters do not own separate collections. They read and write the same
`markets`, `measurement_points`, and `timeseries` records as the canonical API.

## Scheduler Behavior

`update_db_periodically` periodically marks stale open orders as `Expired` using `time_slot` and current time.

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
- `UPDATE_INTERVAL`
