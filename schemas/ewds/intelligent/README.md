# Intelligent EWDS Data Schemas

This folder contains JSON Schemas for GSY DEX off-chain communication over
EWDS.

Identifier and order-event schema regression tests run in Docker:

```bash
docker run --rm -v "$PWD":/work -w /work python:3.12-slim \
  sh -c 'pip install --quiet jsonschema==4.25.1 && python -m unittest discover -s schemas/ewds/tests -v'
```

These validate local schemas, not the schema version installed in a gateway.

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
- `int.markets.query.request.v1.json` for market/community filters; malformed or
  nil IDs receive `invalid_request`. Market responses and events use UUID IDs,
  requiring UUIDv4. Stored market IDs use the same lowercase UUID text.
- `int.facilities.validate.request.v1.json` / `int.facilities.validate.response.v1.json`
  for read-only facility/market eligibility (`facilitiesValidate` / `facilitiesValidateResponse`)
- `int.ids.query.request.v1.json` and `int.clearing_results.query.request.v1.json`
  for get-or-create ID mappings and clearing-result reads
- Historical upsert envelopes for forecasts, measurements, and markets have no
  active EWDS request handler. Use the documented HTTP routes or supported events;
  registering a schema alone does not enable an operation.

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
- Entity IDs use hyphenated UUIDv4 and timestamps use ISO 8601; actor IDs remain opaque off-chain strings.
- The active DEX EVM matching path uses bytes16 IDs and owner hashes, and
  Unix timeslots. On-chain IDs are not EVM wallet addresses. Market events expose
  UUIDv4s; order lifecycle, trade and clearing events expose the same lowercase UUID format.
  Their actor fields (`createdBy`, preferred partner, buyer/seller) expose the original
  off-chain identities, not hashes. Missing mappings defer the event in storage.
- Communities, sites and facilities carry `valid_from` and `valid_until`
  (`validFrom` and `validUntil` for communities) as RFC 3339 strings in UTC.
  The off-chain storage sets `valid_from` to the time it received the entry. A
  sender sets `valid_until` to delete an entry from that time on. Sites are
  identified by `site_id`, which a facility's `site_id` refers to.
- These schemas are versioned as `v1` and should be registered as EWDS topic
  schemas when the corresponding topics are managed with `topiccreator`.
