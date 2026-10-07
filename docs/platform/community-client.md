# GSY DEX Community Client

## Purpose

`gsy-community-client` is the ingestion bridge for community data and on-chain order
publication.

## Responsibilities

- Pull external community facility topology, forecasts, and measurements.
- Normalize profile data into ontology `MeasurementPoint` and `Timeseries` records.
- Forward market openings as ontology `Market` records.
- Publish bid/offer orders on-chain via `OrderRegistry.placeOrder`.
- Send the orders FOS publishes as EWDS `order.submitted` events to
  `OrderRegistry.placeOrder`.

## Facility Topology and Market Coupling

The client uses the external facility topology to validate forecast and
measurement facility IDs, then stores the market opening in off-chain storage
for the target timeslot.
Market IDs are generated with the same deterministic scheme used by orchestrator.

## Order Publication Logic

For each forecast:

- Positive `energy_kwh` -> publish bid.
- Negative `energy_kwh` -> publish offer.

Order payload includes:

- `owner`
- `createdBy` derived from `facilityId`
- `marketId`
- `timeSlot`
- `creationTime`
- scaled `energy`
- scaled `energyRate`

The EVM order tuple also supports optional requirements for bids and offers
(`energySourcePreference`, `preferredTradingPartner`, `preferredEnergyRate`)
and offer attributes (`energyType`). Missing optional values are encoded with
zero-value sentinels.

## Order Events

When `EWDS_ENABLE_HANDLER` is on, the binary polls the `order.submitted` topic
(`EWDS_ORDER_SUBMITTED_EVENT_TOPIC`, default `orderSubmitted`) on
`EWDS_EVENT_SUBSCRIBE_FQCN`, and no other topic. For every event it:

1. parses and checks all orders (UUID `orderId`, `orderStatus` `submitted`,
   `quantity` above 0, `priceLimit` 0 or more) and resolves `createdBy` and the
   trading partners to on-chain IDs through the ID service. One invalid order
   rejects the whole event before anything is sent;
2. sends the orders to `OrderRegistry.placeOrder` one at a time, skipping
   orders that are already on-chain. An order the contract would reject is
   logged and not sent, and the others are still sent.

An event that fails for another reason than invalid data, e.g. because the
node or the ID service can't be reached, is retried with a growing delay
(`EWDS_EVENT_HANDLE_ATTEMPTS`, `EWDS_EVENT_RETRY_DELAY_MS`), and later events
wait for it.

It doesn't wait for the transactions to be mined, so a revert in a mined
transaction goes unnoticed. Each transaction takes its nonce from the pending
transaction count. The event contract and all handling rules are in
[EWDS Integration](ewds-integration.md#order-events).

The community client's wallet must be authorized for every actor it sends
orders for (`ActorRegistry.isAuthorized`); otherwise the contract rejects the
order with `Unauthorized`.

## Configuration

- `EVM_NODE_URL` (default `ws://anvil:8545`)
- `ORDER_REGISTRY_ADDRESS` (required for order events)
- `COMMUNITY_CLIENT_PRIVATE_KEY` (required for order events)
- `EWDS_ENABLE_HANDLER`: starts the order event subscriber
- `EWDS_COMMUNITY_CLIENT_ID` (default `gsycommunityclient`): EWDS client ID,
  also used for ID service queries over EWDS
- `EWDS_ORDER_SUBMITTED_EVENT_TOPIC`, `EWDS_EVENT_SUBSCRIBE_FQCN`,
  `EWDS_EVENT_BATCH_SIZE` (default 100), `EWDS_EVENT_POLL_INTERVAL_MS`
  (default 60 000 ms, 1 000 ms in the e2e stack), `EWDS_EVENT_HANDLE_ATTEMPTS`
  (default 8), `EWDS_EVENT_RETRY_DELAY_MS` (default 2 000 ms) and the shared
  EWDS gateway settings
- `OFFCHAIN_STORAGE_TRANSPORT` / `OFFCHAIN_STORAGE_URL` for the ID service
- external source URLs for facility topology/forecasts/measurements

## Operational Notes

The binary currently runs only the order event subscriber. Without
`EWDS_ENABLE_HANDLER` it logs that the subscriber is disabled and exits.
The forecast-based order publication (`publish_orders`) is used by the e2e
steps.
