# Intelligent EWDS Data Schemas

This folder contains draft JSON Schemas derived from the Intelligent ontology CSV
for GSY DEX off-chain communication over EWDS.

Scope of this first schema pack:

- Domain entities: `Order`, `Trade`, `Tariff`, `GridFeeModel`, `MarketMechanism`
- Message contracts: query request/response envelopes for orders, trades, measurements

Notes:

- CSV ontology property names are kept in camelCase where practical (`tradeId`, `marketId`).
- Runtime service fields from GSY DEX are mapped in `docs/platform/ewds-data-contracts.md`.
- Date/time fields use transitional `oneOf` (`string` date-time or unix seconds integer)
  to support current service payloads.
- These schemas are versioned as `v1` and should be registered as EWDS topic schemas.
