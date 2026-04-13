# GSY DEX Off-Chain Storage

## Purpose

`gsy-orderbook-service` is the off-chain state API and persistence layer.  
It stores indexed on-chain events and supporting data (forecasts, measurements, markets).

Backend: MongoDB (`mongo:5.0`).

## Event Indexing Path

1. `gsy-ethers-listener` subscribes to:
   - `OrderPlaced`
   - `OrderCancelled`
   - `TradeSettled`
   - `MarketStatusUpdated`
2. `OrderbookEvmHandler` maps event payloads into DB schemas.
3. `gsy-orderbook-service` updates order/trade records and exposes them via REST APIs.

## HTTP API Surface

- `/health_check`
- `/orders` (`GET`, `POST`)
- `/trades` (`GET`, `POST`)
- `/measurements` (`GET`, `POST`)
- `/forecasts` (`GET`, `POST`)
- `/market` (`GET`, `POST`)
- `/community-market` (`GET`)
- `/asset-measurements` (`GET`, `POST`)

## Scheduler Behavior

`start_scheduler` periodically marks stale open orders as `Expired` using `time_slot` and current time.

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
