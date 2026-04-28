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

## How These Schemas Are Used in EWDS

Planned usage:

1. Register message topics in EWDS client gateway.
2. Attach request/response schemas to each topic version.
3. Enforce validators on producer and consumer boundaries.
4. Reject malformed payloads before business logic execution.

## Open Decisions

- Final namespace and owner naming conventions in Intelligent EWDS.
- Whether to normalize all timestamps to RFC3339 before full cutover.
- Whether trade IDs should be UUID-only (strict pattern) or generic strings.
- Final representation for market types (`spot`, `flexibility`, `settlement`) across services.

## Next Implementation Step

Register the schema pack as DDHub topic versions and align the Intelligent namespace
and channel policies so the implemented request/response handlers can be validated
against the active gateway runtime.
