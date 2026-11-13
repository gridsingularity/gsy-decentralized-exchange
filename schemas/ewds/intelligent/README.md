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

- query request/reply envelopes for orders, trades, measurements, and communities
- `int.facilities.validate.request.v1.json` / `int.facilities.validate.response.v1.json`
  for read-only facility/market eligibility (`facilitiesValidate` / `facilitiesValidateResponse`)
- upsert request envelopes for forecasts, measurements, and markets

Event envelopes wrap a non-empty list of the entities that other systems send
to GSY on the events channels, one per topic:

- `int.measurements.event.v1.json` (`int.measurement.schema.v1.json`)
- `int.facility.event.v1.json` (`int.facility.schema.v1.json`)
- `int.site.event.v1.json` (`int.site.schema.v1.json`)
- `int.community.event.v1.json` (`int.community.schema.v1.json`)
- `int.order.event.v1.json` (`int.order.schema.v1.json`), consumed
  by the community client for `order.event` submissions. The same schema and
  topic also accept `orderStatus.event` notifications for FOS, which the
  community client ignores. Off-chain storage publishes confirmed lifecycle
  updates after indexing when the EWDS handler is enabled. The community client
  reports definite contract rejections on the same topic without storing orders;
  notifications for malformed submissions are outside DD-423; those errors
  remain in logs pending a separate message-format decision.

The off-chain storage publishes these event envelopes on the events channels:

- `int.order.event.v1.json` (`int.order.schema.v1.json`), with `orderStatus.event`
- `int.trade.event.v1.json` (`int.trade.schema.v1.json`)
- `int.clearing-result.event.v1.json` (`int.clearing-result.schema.v1.json`)
- `int.market.event.v1.json` (`int.market.schema.v1.json`)

Notes:

- Intelligent ontology property names are kept in camelCase (`tradeId`, `marketId`).
- Facility-validation pairs/results use the shared HTTP API's `facility_id` and
  `market_id` names; these are validation records, not ontology entity DTOs.
- Runtime service fields from GSY DEX are mapped in `docs/platform/ewds-data-contracts.md`.
- The agreed Intelligent schemas use UUID identifiers and ISO 8601 timestamps.
- The active DEX EVM settlement path still uses separate runtime DTOs for
  bytes32 hashes, area UUID hashes, and unix timeslots.
- These schemas are versioned as `v1` and should be registered as EWDS topic
  schemas when the corresponding topics are managed with `topiccreator`.
