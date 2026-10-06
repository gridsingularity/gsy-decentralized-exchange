# GSY DEX Community Client

## Purpose

`gsy-community-client` is the ingestion bridge for community data and on-chain order
publication.

## Responsibilities

- Pull external community facility topology, forecasts, and measurements.
- Normalize profile data into ontology `MeasurementPoint` and `Timeseries` records.
- Publish bid/offer orders on-chain via `OrderRegistry.placeOrder`.

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

Order payload includes:

- `owner`
- `createdBy` derived from `facilityId`
- `marketId`
- `timeSlot`
- `creationTime`
- scaled `energy`
- scaled `energyRate`

The EVM order tuple also supports optional bid requirements
(`energySourcePreference`, `preferredTradingPartner`, `preferredEnergyRate`)
and offer attributes (`energyType`, `tradingPartner`). Missing optional values
are encoded with zero-value sentinels.

## Configuration

- `EVM_NODE_URL`
- `ORDER_REGISTRY_ADDRESS`
- `COMMUNITY_CLIENT_PRIVATE_KEY`
- external source URLs for facility topology/forecasts/measurements

## Operational Notes

The service is polling-based and forwards data continuously.  
If no valid data is found for a cycle, it logs and continues without failing hard.
