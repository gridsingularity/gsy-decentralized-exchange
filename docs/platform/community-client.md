# GSY DEX Community Client

## Purpose

`gsy-community-client` is the ingestion bridge for community data and on-chain order
publication.

## Responsibilities

- Pull external community topology, forecasts, and measurements.
- Normalize and forward profile data to Orderbook REST APIs.
- Publish bid/offer orders on-chain via `OrderRegistry.placeOrder`.

## Topology and Market Coupling

The client resolves or creates market topology in Orderbook for the target timeslot.
Market IDs are generated with the same deterministic scheme used by orchestrator.

## Order Publication Logic

For each forecast:

- Positive `energy_kwh` -> publish bid.
- Negative `energy_kwh` -> publish offer.

Order payload includes:

- `owner`
- `nonce`
- `areaUuid`
- `marketId`
- `timeSlot`
- `creationTime`
- scaled `energy`
- scaled `energyRate`

## Configuration

- `EVM_NODE_URL`
- `ORDER_REGISTRY_ADDRESS`
- `COMMUNITY_CLIENT_PRIVATE_KEY`
- external source URLs for topology/forecasts/measurements

## Operational Notes

The service is polling-based and forwards data continuously.  
If no valid data is found for a cycle, it logs and continues without failing hard.
