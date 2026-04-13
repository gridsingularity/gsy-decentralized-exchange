# GSY DEX Market Orchestrator

## Purpose

`gsy-market-orchestrator` controls market open/close transitions on-chain through
`MarketController`.

## Core Behavior

1. Wait until orchestrator signer has `ORCHESTRATOR_ROLE`.
2. Run periodic ticks (`tick_interval_seconds`).
3. For each delivery slot in look-ahead horizon:
   - Compute deterministic `marketId`.
   - Determine expected open/close state from configured offsets.
   - Compare expected state with on-chain status.
   - Submit `setMarketStatus` only when state transition is required.

## Deterministic Market ID

`marketId` is generated from:

- `MarketType` string (`Spot`, `Flexibility`, `Settlement`)
- Delivery timestamp (`u64`)
- Blake2b hash, 32-byte output

This allows all services to derive consistent identifiers.

## Configurable Parameters

- `EVM_NODE_URL`
- `MARKET_CONTROLLER_ADDRESS`
- `ORCHESTRATOR_SIGNER_PRIVATE_KEY`
- `TICK_INTERVAL_SECONDS`
- `LOOK_AHEAD_HOURS`
- Market window offsets (via global constants/env)

## Failure Handling

Each tick is isolated. Errors are logged and next tick continues, so transient failures
do not stop orchestration permanently.
