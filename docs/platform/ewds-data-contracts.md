# EWDS Data Contracts (Intelligent Ontology Alignment)

## Purpose

This page translates the Intelligent ontology spreadsheet into concrete JSON data
contracts for GSY DEX inter-service communication over EWDS.

Source input: ontology CSV definitions for classes and properties such as
`int:Trade`, `int:Order`, `int:Tariff`, `int:GridFeeModel`, and related attributes.

## Schema Package Location

Schema files are versioned in:

- `schemas/ewds/intelligent/`

Primary files:

- `int.order.schema.v1.json`
- `int.trade.schema.v1.json`
- `int.measurement.schema.v1.json`
- `int.forecast.schema.v1.json`
- `int.market.schema.v1.json`
- `int.orders.query.request.v1.json`
- `int.orders.query.response.v1.json`
- `int.trades.query.request.v1.json`
- `int.trades.query.response.v1.json`
- `int.measurements.query.request.v1.json`
- `int.measurements.query.response.v1.json`
- `int.forecasts.upsert.request.v1.json`
- `int.measurements.upsert.request.v1.json`
- `int.market.upsert.request.v1.json`

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

## CSV -> Runtime Field Mapping

### Trade Mapping

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:tradeId` | `tradeId` | `TradeSchema.trade_uuid` |
| `int:bidId` | `bidId` | `TradeSchema.bid_hash` |
| `int:offerId` | `offerId` | `TradeSchema.offer_hash` |
| `int:residualBidId` | `residualBidId` | `TradeSchema.residual_bid.order_id` |
| `int:residualOfferId` | `residualOfferId` | `TradeSchema.residual_offer.order_id` |
| `int:marketId` | `marketId` | `TradeSchema.market_id` |
| `int:tradeStatus` | `tradeStatus` | `TradeSchema.status` |
| `int:tradeQuantity` | `tradeQuantity` | `TradeSchema.parameters.selected_energy_kWh` |
| `int:tradePrice` | `tradePrice` | `TradeSchema.parameters.energy_rate` |
| `int:tradeTimestamp` | `tradeTimestamp` | `TradeSchema.creation_time` |
| `int:buyer` | `buyer` | `TradeSchema.buyer` |
| `int:seller` | `seller` | `TradeSchema.seller` |

### Order Mapping

| Ontology property | Schema field | Current runtime source |
|---|---|---|
| `int:orderId` | `orderId` | `DbOrderSchema.order_id` |
| `int:marketId` | `marketId` | `DbOrderSchema.market_id` |
| runtime routing field | `areaUuid` | `DbOrderSchema.area_uuid` |
| runtime settlement field | `nonce` | `DbOrderSchema.nonce` |
| `int:orderType` | `orderType` | `DbOrderSchema.order_type` |
| `int:quantity` | `quantity` | `DbOrderSchema.energy_kWh` |
| `int:priceLimit` | `priceLimit` | `DbOrderSchema.energy_rate` |
| `int:timeSlot` | `timeSlot` | `DbOrderSchema.time_slot` |
| `int:createdBy` | `createdBy` | `DbOrderSchema.created_by` |

## Validation Rules Applied from CSV

The first schema pack encodes the following validation intent from the spreadsheet:

- Type-safe fields for trade and order identifiers.
- Required `tradeId` for trade objects.
- Numeric constraints for quantities and prices (`minimum: 0`).
- Explicit enums for status and order type.
- Transitional date handling (`date-time` or unix seconds integer) to match current runtime.

## Sensitivity and Anonymization Baseline

The provided CSV marks these pilot-level fields as non-sensitive in the current draft.

Current implementation baseline:

- No anonymization transform is applied in schema validation.
- Payload-level minimization is still recommended for EWDS transport (send only required fields).
- If sensitivity flags are updated in the ontology spreadsheet, schema contracts should be versioned.

## Open Decisions

- Whether to normalize all timestamps to RFC3339 before full cutover.
- Whether trade IDs should be UUID-only (strict pattern) or generic strings.
- Final representation for market types (`spot`, `flexibility`, `settlement`) across services.
