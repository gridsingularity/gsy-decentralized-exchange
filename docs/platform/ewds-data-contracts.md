# EWDS Data Contracts (Intelligent Ontology Alignment)

## Purpose

This page translates the Intelligent ontology schemas into concrete JSON data
contracts for GSY DEX inter-service communication over EWDS.

Source input: Energy Web Intelligent schemas and the agreed ontology workbook,
including `Community`, `Market`, `MarketTimeSeries`, `Order`, `Trade`, and
`ClearingResult`.

## Identifier Syntax

The shared bytes16 parser accepts hyphenated UUIDs with the exact 8-4-4-4-12
structure, `0x` followed by 32 hex digits, or bare 32 hex digits. Hex digits may
be upper- or lowercase. Whitespace, uppercase `0X`, braces, `urn:uuid:` notation,
misplaced hyphens and `0x`-prefixed hyphenated UUIDs are rejected. UUID version
and variant bits are unrestricted: market IDs are hashes, not necessarily v4.
Order submissions require non-zero order and market IDs. The raw byte parser
still accepts zero for optional contract sentinels.

The shared order schema serves both submissions and status notifications.
Lifecycle notifications contain lowercase `0x` order IDs and on-chain facility
hashes in `createdBy`; rejections retain the submitted identifiers. FOS must
correlate equivalent order IDs by their 16 bytes, not their spelling.
`preferredTradingPartner` remains an off-chain identity, even when hex-shaped.

Off-chain facility and owner IDs are not required to be UUIDs. Strict hyphenated
UUIDs are lowercased; other nonempty, unpadded strings remain case-sensitive,
including bare/`0x` hex strings. This rule applies to topology, site facility
references, ID mappings, order participants/preferences and measurement/forecast
facility references. Stored entity reads return the canonical spelling;
`facilities.validate` still echoes each requested pair exactly and reports invalid
facility identities as `unknown_facility` per pair. Other invalid typed identities
produce HTTP 400, `invalid_request` for queries, or invalidate the whole inbound
event before writes/submissions (logged, without a new notification).
Names and correlation IDs are not normalized.

## Schema Package Location

Schema files are versioned in:

- `schemas/ewds/intelligent/`

Primary files:

- `int.order.schema.v1.json`
- `int.community.schema.v1.json`
- `int.trade.schema.v1.json`
- `int.clearing-result.schema.v1.json`
- `int.measurement.schema.v1.json`
- `int.forecast.schema.v1.json`
- `int.market.schema.v1.json`
- `int.market-time-series.schema.v1.json`
- `int.orders.query.request.v1.json`
- `int.orders.query.response.v1.json`
- `int.trades.query.request.v1.json`
- `int.trades.query.response.v1.json`
- `int.measurements.query.request.v1.json`
- `int.measurements.query.response.v1.json`
- `int.forecasts.upsert.request.v1.json`
- `int.measurements.upsert.request.v1.json`
- `int.market.upsert.request.v1.json`
- `int.communities.query.request.v1.json`
- `int.communities.query.response.v1.json`
- `int.facility.schema.v1.json`
- `int.site.schema.v1.json`
- `int.measurements.event.v1.json`
- `int.facility.event.v1.json`
- `int.site.event.v1.json`
- `int.community.event.v1.json`
- `int.order.event.v1.json`
- `int.trade.event.v1.json`
- `int.clearing-result.event.v1.json`
- `int.market.event.v1.json`

DDHub topic names use camelCase because the Client Gateway UI rejects dots in
topic names. The schema file names and payload `operation` values keep dotted
operation names for readability and service routing:

| DDHub topic | Payload operation | Schema file |
|---|---|---|
| `ordersQuery` | `orders.query` | `int.orders.query.request.v1.json` |
| `ordersQueryResponse` | response envelope | `int.orders.query.response.v1.json` |
| `tradesQuery` | `trades.query` | `int.trades.query.request.v1.json` |
| `tradesQueryResponse` | response envelope | `int.trades.query.response.v1.json` |
| `measurementsQuery` | `measurements.query` | `int.measurements.query.request.v1.json` |
| `measurementsQueryResponse` | response envelope | `int.measurements.query.response.v1.json` |
| `communitiesQuery` | `communities.query` | `int.communities.query.request.v1.json` |
| `communitiesQueryResponse` | response envelope | `int.communities.query.response.v1.json` |
| `measurements` | `measurements.event` | `int.measurements.event.v1.json` |
| `facility` | `facility.event` | `int.facility.event.v1.json` |
| `site` | `site.event` | `int.site.event.v1.json` |
| `community` | `community.event` | `int.community.event.v1.json` |
| `order` | `order.event` | `int.order.event.v1.json` |
| `order` | `orderStatus.event` | `int.order.event.v1.json` |
| `trade` | `trade.event` | `int.trade.event.v1.json` |
| `clearingResult` | `clearingResult.event` | `int.clearing-result.event.v1.json` |
| `market` | `market.event` | `int.market.event.v1.json` |

### Measurement Mapping

Used by `measurements.query` responses and `measurements.event` messages.

| Schema field | Current runtime source |
|---|---|
| `facilityId` | `MeasurementSchema.facility_id` |
| `communityUuid` | `MeasurementSchema.community_uuid` |
| `timeSlot` | `MeasurementSchema.time_slot` (RFC 3339) |
| `creationTime` | `MeasurementSchema.creation_time` (RFC 3339) |
| `energyKwh` | `MeasurementSchema.energy_kwh` |

Inbound measurement events require a nonzero `communityUuid` as UUID or bytes16
hex; storage normalizes it to a lowercase hyphenated UUID before generating the
profile-point ID. This is an ingress restriction, not a shared DTO conversion
rule: query responses can still carry an empty community or a datasource label
from generic measurement points. The event schema adds the restriction separately
from the shared measurement schema.

`measurements.query` supports optional `facilityId` and `communityUuid` filters,
combined with inclusive RFC 3339 `startTime`/`endTime` bounds. Existing snake_case
names and `areaUuid` remain accepted. Invalid filters receive `invalid_request`;
specify only one spelling per field. Community UUID/hex variants address the same
canonical community; facility IDs follow the off-chain identity rule above.

### Facility and Site Mapping

`facility.event` and `site.event` messages carry lists of
`FacilitySchema` and `SiteSchema` unchanged, so their fields are snake_case
(`facility_id`, `site_name`, ...), like `facilities.query` responses.

### Community Mapping

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:communityId` | `communityId` | `EnergyCommunitySchema.community_id` |
| `int:communityName` | `communityName` | `EnergyCommunitySchema.community_name` |
| site membership | `sites` | `EnergyCommunitySchema.sites` |

### Trade Mapping

DD-453 changes the v1 `tradeStatus` values in place to `matched`, `executed`,
`settled` and `penalized`; no new schema version was introduced. Consumers must
use these values and the current lifecycle: listener-indexed block inclusion
creates `matched`, verification produces `executed` or terminal `penalized`,
and financial `settled` is reserved for a future integration.

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:tradeId` | `tradeId` | `DbTradeSchema.trade_uuid` |
| `int:bidId` | `bidId` | `DbTradeSchema.bid_hash` |
| `int:offerId` | `offerId` | `DbTradeSchema.offer_hash` |
| `int:residualBidId` | `residualBidId` | `DbTradeSchema.residual_bid_id` |
| `int:residualOfferId` | `residualOfferId` | `DbTradeSchema.residual_offer_id` |
| `int:marketId` | `marketId` | `DbTradeSchema.market_id` |
| `int:tradeStatus` | `tradeStatus` | `DbTradeSchema.status` |
| `int:tradeQuantity` | `tradeQuantity` | `DbTradeSchema.parameters.selected_energy_kWh` |
| `int:tradePrice` | `tradePrice` | `DbTradeSchema.parameters.energy_rate` |
| `int:tradedAt` | `timestamp` | `DbTradeSchema.creation_time` (RFC 3339) |
| `int:buyerId` | `buyerId` | `DbTradeSchema.buyer` |
| `int:sellerId` | `sellerId` | `DbTradeSchema.seller` |

### Order Mapping

Used by `orders.query` responses and `order.event` messages. Times are
RFC 3339 strings on the wire.

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:orderId` | `orderId` | `DbOrderSchema.order_id` |
| `int:marketId` | `marketId` | `DbOrderSchema.market_id` |
| `int:orderType` | `orderType` | `DbOrderSchema.order_type` |
| `int:orderStatus` | `orderStatus` | `DbOrderSchema.status` |
| `int:quantity` | `quantity` | `DbOrderSchema.energy_kWh` |
| `int:priceLimit` | `priceLimit` | `DbOrderSchema.energy_rate` |
| `int:timeSlot` | `timeSlot` | `DbOrderSchema.time_slot` |
| `int:createdBy` | `createdBy` | `DbOrderSchema.created_by` |
| `int:createdAt` | `creationTime` | `DbOrderSchema.creation_time` |

For an `order.event`, the community client builds the
`OrderRegistry.placeOrder` parameters like this:

| Contract parameter | From |
|---|---|
| `orderId` | the 16 bytes of the `orderId` UUID |
| `createdBy` | Facility ID, mapped to its on-chain ID by the ID service; not the owner ID or service wallet |
| `marketId` | the 16 bytes of `marketId` |
| `timeSlot`, `creationTime` | `timeSlot`, `creationTime` as epoch seconds |
| `energy`, `energyRate` | `quantity`, `priceLimit` × 10 000, rounded |
| `isBid` | `orderType` is `bid` |
| `energySourcePreference`, `preferredTradingPartner`, `preferredEnergyRate` | requirements for bids and offers (`energySourcePreference`, `preferredTradingPartner` mapped by the ID service, `preferredEnergyRate` × 10 000) |
| `energyType` | the offer attributes (`energyType`) |

Order and market IDs represent bytes16, not bytes32. Lifecycle `createdBy` is
the bytes16 facility hash, not an EVM wallet address. The shared order DTO also
serves legacy HTTP imports, which require this already-resolved facility hash;
`order.event` submissions instead carry the off-chain facility identity.

## Validation Rules Applied from CSV

The first schema pack encodes the following validation intent from the spreadsheet:

- UUID fields for Intelligent identifiers.
- ISO 8601 date-time fields for market, order, trade, clearing, measurement and
  event timestamps.
- Positive numeric constraints for quantities.
- Explicit enums for market type, matching algorithm, order type/status, trade
  status, clearing status, and no-bid reason.

## Sensitivity and Anonymization Baseline

The provided CSV marks these pilot-level fields as non-sensitive in the current draft.

Current implementation baseline:

- No anonymization transform is applied in schema validation.
- Payload-level minimization is still recommended for EWDS transport (send only required fields).
- If sensitivity flags are updated in the ontology spreadsheet, schema contracts should be versioned.
