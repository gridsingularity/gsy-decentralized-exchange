# EWDS Data Contracts (Intelligent Ontology Alignment)

## Purpose

This page translates the Intelligent ontology schemas into concrete JSON data
contracts for GSY DEX inter-service communication over EWDS.

Source input: Energy Web Intelligent schemas and the agreed ontology workbook,
including `Community`, `Market`, `MarketTimeSeries`, `Order`, `Trade`, and
`ClearingResult`.

## Identifier Syntax

Order, residual-order, trade and market IDs must be hyphenated UUIDv4 values
(exact 8-4-4-4-12 structure, version 4, RFC variant). Either letter case is
accepted; storage, reads and lifecycle events use lowercase UUID text.
Hex/bare IDs, whitespace, braces, URNs, nil and other versions are rejected.
These entity IDs convert directly to/from their 16 bytes at the contract boundary,
without hashing. Optional residuals use null off-chain and zero bytes16 on-chain.

The shared order schema serves both submissions and status notifications.
Lifecycle notifications contain lowercase UUID order IDs and the original
off-chain owner ID in `createdBy`; rejections retain the submitted identifiers. FOS must
correlate equivalent order IDs by their 16 bytes, not their spelling.
`preferredTradingPartner` remains an off-chain identity, even when hex-shaped.

Off-chain facility, owner and community IDs are not required to be UUIDs. All nonempty,
unpadded strings are preserved exactly and remain case-sensitive, including
UUID-shaped and bare/`0x` hex strings. This rule applies to topology, site facility
references, ID mappings, order participants/preferences and measurement/forecast
facility/community references and query filters. Stored actor/community reads return the submitted spelling;
`facilities.validate` still echoes each requested pair exactly and reports invalid
facility identities as `unknown_facility` per pair. Other invalid typed identities
produce HTTP 400, `invalid_request` for queries, or invalidate the whole inbound
event before writes/submissions (logged, without a new notification).
Names and correlation IDs are not normalized.

| Message/read | Identity representation |
|---|---|
| `market.event`, `markets.query` | Market ID as a lowercase hyphenated UUID; community ID as the exact opaque off-chain string |
| `order.event` | UUIDv4 order and market IDs; off-chain owner identity and preferred partner |
| `orderStatus.event` lifecycle | Lowercase UUIDv4 order/market IDs; off-chain owner and preferred partner |
| `orderStatus.event` rejection | Submitted identifiers echoed without rewriting |
| `trade.event`, `trades.query` | Lowercase UUIDv4 trade/market/order IDs; off-chain buyer/seller IDs; absent residuals are null |
| `clearingResult.event`, `clearing_results.query` | Lowercase UUIDv4 market ID |
| `ids.query` | Exact off-chain identity and its lowercase `0x` keyed hash |
| `facilities.validate` | Exact request pairs, preserving order and cardinality |

Deploy all services from the same DD-459 commit into a fresh environment. The
ID service preserves actor identities and clients compare their exact strings.
This does not migrate old mappings or relabel persisted records.

Actor hashes use keyed Blake2b with 16-byte output and the shared
`ACTOR_ID_HASH_KEY`; they are pseudonyms, not reversible encryption or UUIDv4.
Topology writes register mappings for facilities, owners, site keys and exact
community IDs. Market IDs are generated as UUIDv4 only when tuple lookup reports
an absent market; stored IDs use lowercase UUID text. The indexer restores the
community's exact off-chain ID by direct DB reverse lookup before publication.
Missing actor mappings leave the original chain event in a durable pending collection.
Dependent events wait in chain order; unrelated markets continue indexing. Resolved
order/trade records and notifications use off-chain owner IDs. Storage retries pending
events on subsequent blocks, including after restart. Queries touching known-incomplete
projections fail explicitly rather than returning a partial order book or trade set.
Existing `POST /ids` and `ids.query` callers remain supported. API authentication
is deferred to a separate ticket.

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
- `int.markets.query.request.v1.json`
- `int.ids.query.request.v1.json`
- `int.clearing_results.query.request.v1.json`
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

The three `*.upsert.request` schemas above are historical contracts without
active request handlers. They do not enable EWDS writes; use HTTP profile/market
routes or supported inbound events instead.

DDHub topic names use camelCase because the Client Gateway UI rejects dots in
topic names. The schema file names and payload `operation` values keep dotted
operation names for readability and service routing:

| DDHub topic | Payload operation | Schema file |
|---|---|---|
| `ordersQuery` | `orders.query` | `int.orders.query.request.v1.json` |
| `ordersQueryResponse` | response envelope | `int.orders.query.response.v1.json` |
| `tradesQuery` | `trades.query` | `int.trades.query.request.v1.json` |
| `tradesQueryResponse` | response envelope | `int.trades.query.response.v1.json` |
| `marketsQuery` | `markets.query` | `int.markets.query.request.v1.json` |
| `idsQuery` | `ids.query` | `int.ids.query.request.v1.json` |
| `clearingResultsQuery` | `clearing_results.query` | `int.clearing_results.query.request.v1.json` |
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

Inbound measurement events require `communityUuid` to be a nonempty, unpadded
opaque string. Storage preserves its exact spelling in the profile-point ID,
including UUID-looking values. This is an ingress restriction, not a shared DTO conversion
rule: query responses can still carry an empty community or a datasource label
from generic measurement points. The event schema adds the restriction separately
from the shared measurement schema.

`measurements.query` supports optional `facilityId` and `communityUuid` filters,
combined with inclusive RFC 3339 `startTime`/`endTime` bounds. Existing snake_case
names and `areaUuid` remain accepted. Invalid filters receive `invalid_request`;
specify only one spelling per field. Community and facility filters match exact
strings; UUID/hex-looking variants or differently cased values are distinct identities.

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
and `settled` means the trade has been processed in the Settlement market.
The transition to `settled` is not yet implemented.

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
| `createdBy` | Facility owner ID, hashed locally with the shared actor key; not the facility ID or service wallet |
| `marketId` | the 16 bytes of `marketId` |
| `timeSlot`, `creationTime` | `timeSlot`, `creationTime` as epoch seconds |
| `energy`, `energyRate` | `quantity`, `priceLimit` × 10 000, rounded |
| `isBid` | `orderType` is `bid` |
| `energySourcePreference`, `preferredTradingPartner`, `preferredEnergyRate` | requirements for bids and offers (`energySourcePreference`, registered preferred owner hashed locally, `preferredEnergyRate` × 10 000) |
| `energyType` | the offer attributes (`energyType`) |

Order and market IDs represent bytes16, not bytes32. `createdBy` and preferred
partners in submissions, imports, queries and lifecycle notifications are original
off-chain owner IDs, not hashes or wallet addresses. Actor hashes are used only
for contract interaction and retained in the private mapping/raw pending events.
An explicitly supplied preferred partner must be an owner with a registered
facility; otherwise the client sends an `unknown_preferred_partner` rejection
without placing that order. This check does not validate partner market eligibility.

## Validation Rules Applied from CSV

The first schema pack encodes the following validation intent from the spreadsheet:

- UUIDv4 fields for lifecycle entity IDs; actor/community IDs follow the
  exact opaque-string rules above.
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
