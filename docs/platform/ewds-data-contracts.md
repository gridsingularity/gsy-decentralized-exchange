# EWDS Data Contracts (Intelligent Ontology Alignment)

## Purpose

This page translates the Intelligent ontology schemas into concrete JSON data
contracts for GSY DEX inter-service communication over EWDS.

Source input: Energy Web Intelligent schemas and the agreed ontology workbook,
including `Community`, `Market`, `MarketTimeSeries`, `Order`, `Trade`, and
`ClearingResult`.

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
- `int.measurements.submitted.event.v1.json`
- `int.facility.submitted.event.v1.json`
- `int.site.submitted.event.v1.json`
- `int.community.submitted.event.v1.json`

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
| `measurementsSubmitted` | `measurements.submitted` event | `int.measurements.submitted.event.v1.json` |
| `facilitySubmitted` | `facility.submitted` event | `int.facility.submitted.event.v1.json` |
| `siteSubmitted` | `site.submitted` event | `int.site.submitted.event.v1.json` |
| `communitySubmitted` | `community.submitted` event | `int.community.submitted.event.v1.json` |

### Measurement Mapping

Used by `measurements.query` responses and `measurements.submitted` events.

| Schema field | Current runtime source |
|---|---|
| `facilityId` | `MeasurementSchema.facility_id` |
| `communityUuid` | `MeasurementSchema.community_uuid` |
| `timeSlot` | `MeasurementSchema.time_slot` (RFC 3339) |
| `creationTime` | `MeasurementSchema.creation_time` (RFC 3339) |
| `energyKwh` | `MeasurementSchema.energy_kwh` |

### Facility and Site Mapping

`facility.submitted` and `site.submitted` events carry lists of
`FacilitySchema` and `SiteSchema` unchanged, so their fields are snake_case
(`facility_id`, `site_name`, ...), like `facilities.query` responses.

### Community Mapping

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:communityId` | `communityId` | `EnergyCommunitySchema.community_id` |
| `int:communityName` | `communityName` | `EnergyCommunitySchema.community_name` |
| site membership | `sites` | `EnergyCommunitySchema.sites` |

### Trade Mapping

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:tradeId` | `tradeId` | `DbTradeSchema.trade_uuid` |
| `int:bidId` | `bidId` | `DbTradeSchema.bid_hash` |
| `int:offerId` | `offerId` | `DbTradeSchema.offer_hash` |
| `int:residualBidId` | `residualBidId` | `DbTradeSchema.residual_bid.order_id` |
| `int:residualOfferId` | `residualOfferId` | `DbTradeSchema.residual_offer.order_id` |
| `int:marketId` | `marketId` | `DbTradeSchema.market_id` |
| `int:tradeStatus` | `tradeStatus` | `DbTradeSchema.status` |
| `int:tradeQuantity` | `tradeQuantity` | `DbTradeSchema.parameters.selected_energy_kWh` |
| `int:tradePrice` | `tradePrice` | `DbTradeSchema.parameters.energy_rate` |
| `int:tradedAt` | `tradedAt` | `DbTradeSchema.creation_time` |
| `int:buyerId` | `buyerId` | `DbTradeSchema.buyer` |
| `int:sellerId` | `sellerId` | `DbTradeSchema.seller` |

### Order Mapping

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
| `int:createdAt` | `createdAt` | `DbOrderSchema.creation_time` |

Runtime-only fields not represented in the agreed Intelligent `Order` schema:

- `facilityId`
- EVM bytes32 `orderId` / `marketId` hash format
- EVM account address in `createdBy`

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
