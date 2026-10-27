# GSY DEX Community Client

## Purpose

`gsy-community-client` is the ingestion bridge for community data and on-chain order
publication.

## Responsibilities

- Pull external community facility topology, forecasts, and measurements.
- Normalize profile data into ontology `MeasurementPoint` and `Timeseries` records.
- Publish bid/offer orders on-chain via `OrderRegistry.placeOrder`.
- Send the orders FOS publishes as EWDS `order.event` messages to
  `OrderRegistry.placeOrder`.

## Facility Topology and Market Coupling

The client uses the external facility topology to validate forecast and
measurement facility IDs. It does not create markets: the orchestrator creates
them on-chain and off-chain storage indexes them from `NewMarketCreated`. The
client reads the market for a timeslot from off-chain storage; market IDs use
the same deterministic scheme as the orchestrator.

## Order Publication Logic

For each forecast:

- Positive `energy_kwh` -> publish bid.
- Negative `energy_kwh` -> publish offer.
- Zero energy -> no order and no eligibility query for that forecast.

Before submission, the client validates all non-zero forecasts' facility/market
pairs through HTTP `/facilities/validate` or EWDS `facilities.validate`.
Only eligible facilities are resolved through the ID service. `createdBy` uses
that facility's on-chain ID, not its owner's ID or the submitting wallet.
Owner relationships remain in the topology and are not used as submission identity.

Ineligible forecasts are skipped and logged. When `EWDS_ENABLE_HANDLER` is enabled,
they also produce the existing `orderStatus.event` rejection notification with a
generated order ID, original facility ID and validation reason; no DB order is
created. HTTP-only runs do not require a gateway. Validation/ID-service errors
propagate before any order is sent, allowing the caller to retry. All eligible
orders are prepared before rejection notifications are published. The existing
forecast quantity/rate calculations and transaction submission loop are unchanged.

Order payload includes:

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

When `EWDS_ENABLE_HANDLER` is on, the binary polls the `order` topic for `order.event` messages
(`EWDS_ORDER_EVENT_TOPIC`, default `order`) on
`EWDS_EVENT_SUBSCRIBE_FQCN`, and no other topic. For every event it:

1. parses and checks all orders (UUID `orderId`, non-zero UUID/hex `marketId`,
   `orderStatus` `submitted`, `quantity` above 0, `priceLimit` 0 or more);
2. skips orders already confirmed on-chain, then validates the remaining
   `createdBy` facility IDs against their markets in one bulk request through
   HTTP `/facilities/validate` or EWDS `facilities.validate`;
3. resolves IDs only for eligible orders. `createdBy` is the original facility
   ID, not its owner's ID or the submitting wallet. Having an ID mapping does
   not establish eligibility. Ineligible orders receive a `rejected`
   `orderStatus.event` notification with the validation reason, without a DB
   order or a `placeOrder` call; eligible orders in the batch can continue;
4. sends eligible orders to `OrderRegistry.placeOrder` one at a time, skipping
   orders that are already on-chain, and waits for each mined receipt.
   Rejections during gas estimation and mined reverts are logged; the other
   orders are still processed.

Malformed input or unresolved eligible-order IDs fail the event before any
submission. Unavailable validation or incomplete/inconsistent responses do
the same, without rejection notifications. An event that fails for another
reason than invalid data, e.g. because the node, validation service or ID
service can't be reached, is retried with a growing delay
(`EWDS_EVENT_HANDLE_ATTEMPTS`, `EWDS_EVENT_RETRY_DELAY_MS`), and later events
wait for it.

Confirmed orders are excluded before validation on retry, so a topology change
cannot turn them into rejected orders. Before publishing an eligibility
rejection, the client rechecks the chain in case a competing submission or an
earlier item in the batch already placed that order.

Only a receipt with status `1` confirms placement. A missing receipt or an
RPC failure is an unknown outcome and remains retryable, not a rejection.
Order persistence still belongs to the off-chain storage's EVM listener.
Each transaction takes its nonce from the pending transaction count.
The event contract and all handling rules are in
[EWDS Integration](ewds-integration.md#order-events).

The community client's wallet must hold `OrderRegistry.COMMUNITY_CLIENT_ROLE`;
otherwise placement and cancellation revert with `Unauthorized`. Administrators
can authorize multiple clients and revoke each independently. This role is not
community-scoped: each client validates facility/community membership off-chain.
The submitting wallet is independent of the facility ID in `createdBy`;
actor-wallet registration or delegation grants no order-submission permission.

## Configuration

- `EVM_NODE_URL` (default `ws://anvil:8545`)
- `ORDER_REGISTRY_ADDRESS` (required for order events)
- `COMMUNITY_CLIENT_PRIVATE_KEY` (required for order events)
- `EWDS_ENABLE_HANDLER`: starts the order event subscriber
- `EWDS_COMMUNITY_CLIENT_ID` (default `gsycommunityclient`): EWDS client ID,
  also used for ID service queries over EWDS
- `EWDS_ORDER_EVENT_TOPIC`, `EWDS_EVENT_SUBSCRIBE_FQCN`,
  `EWDS_EVENT_BATCH_SIZE` (default 100), `EWDS_EVENT_POLL_INTERVAL_MS`
  (default 60 000 ms, 1 000 ms in the e2e stack), `EWDS_EVENT_HANDLE_ATTEMPTS`
  (default 8), `EWDS_EVENT_RETRY_DELAY_MS` (default 2 000 ms) and the shared
  EWDS gateway settings
- `OFFCHAIN_STORAGE_TRANSPORT` / `OFFCHAIN_STORAGE_URL` for eligibility and ID queries
- `EWDS_FACILITIES_VALIDATION_REQUEST_TOPIC` / `EWDS_FACILITIES_VALIDATION_RESPONSE_TOPIC`
  for eligibility queries when EWDS transport is selected
- external source URLs for facility topology/forecasts/measurements

## Operational Notes

The binary currently runs only the order event subscriber. Without
`EWDS_ENABLE_HANDLER` it logs that the subscriber is disabled and exits.
The forecast-based order publication (`publish_orders`) is used by the e2e
steps and now also validates facility participation. Downstream facility-identity
alignment and the complete E2E fixture update remain subsequent DD-457 steps.
