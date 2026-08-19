# Intelligent EWDS Data Schemas

This folder contains JSON Schemas for GSY DEX off-chain communication over
EWDS.

The market-domain entity schemas are aligned with the Energy Web Intelligent
schemas under `schemas/v1.0.0/market`:

- `int.market.schema.v1.json`
- `int.market-time-series.schema.v1.json`
- `int.order.schema.v1.json`
- `int.trade.schema.v1.json`
- `int.clearing-result.schema.v1.json`

Additional local message contracts wrap these entities for the current
request/reply topics:

- query request/reply envelopes for orders, trades, and measurements
- upsert request envelopes for forecasts, measurements, and markets

Notes:

- Intelligent ontology property names are kept in camelCase (`tradeId`, `marketId`).
- Runtime service fields from GSY DEX are mapped in `docs/platform/ewds-data-contracts.md`.
- The agreed Intelligent schemas use UUID identifiers and ISO 8601 timestamps.
- The active DEX EVM settlement path still uses separate runtime DTOs for
  bytes32 hashes, area UUID hashes, and unix timeslots.
- These schemas are versioned as `v1` and should be registered as EWDS topic
  schemas when the corresponding topics are managed with `topiccreator`.
