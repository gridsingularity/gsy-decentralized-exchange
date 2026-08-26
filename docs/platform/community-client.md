# GSY DEX Community Client

## Purpose

`gsy-community-client` is the ingestion bridge for community data and on-chain order
publication.

## Responsibilities

- Pull external community facility topology, forecasts, and measurements.
- Normalize profile data into ontology `MeasurementPoint` and `Timeseries` records.
- Forward market openings as ontology `Market` records.
- Publish bid/offer orders on-chain via `OrderRegistry.placeOrder`.

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

## Configuration

- `EVM_NODE_URL`
- `ORDER_REGISTRY_ADDRESS`
- `COMMUNITY_CLIENT_PRIVATE_KEY`
- external source URLs for facility topology/forecasts/measurements

## Operational Notes

The service is polling-based and forwards data continuously.  
If no valid data is found for a cycle, it logs and continues without failing hard.
