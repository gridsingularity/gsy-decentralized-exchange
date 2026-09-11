# GSY DEX Market Orchestrator

## Purpose

`gsy-market-orchestrator` controls market open/close transitions on-chain through
`MarketController`.

## Core Behavior

1. Wait until orchestrator signer has `ORCHESTRATOR_ROLE`.
2. Run periodic ticks (`tick_interval_seconds`).
3. Fetch the current communities from off-chain storage through the configured
   HTTP or EWDS transport.
4. For each community, market type, and delivery slot in the look-ahead horizon:
   - Compute deterministic `marketId`.
   - Determine expected open/close state from configured offsets.
   - Compare expected state with on-chain status.
5. Submit required open and close transitions through batched
   `setMarketStatuses` calls.

Communities are fetched on every tick so pilot-site updates are observed without
restarting the orchestrator. A tick is skipped when no communities exist.

## Deterministic Market ID

`marketId` is generated from:

- Community UUID string
- `MarketType` string (`Spot`, `Flexibility`, `Settlement`)
- Delivery timestamp (`u64`)
- Blake2b hash, 16-byte output

The shared generator lives in `primitives::utils::generate_market_id`, allowing
all services to derive the same contract-compatible identifier.

## Configurable Parameters

- `EVM_NODE_URL`
- `MARKET_CONTROLLER_ADDRESS`
- `ORCHESTRATOR_SIGNER_PRIVATE_KEY`
- `TICK_INTERVAL_SECONDS`
- `LOOK_AHEAD_HOURS`
- `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
- `OFFCHAIN_STORAGE_URL`
- `EWDS_MARKET_ORCHESTRATOR_CLIENT_ID`
- Market window offsets (via global constants/env)

## Failure Handling

Each tick is isolated. Community-source, RPC, and transaction errors are logged
and the next tick continues, so transient failures do not stop orchestration
permanently. Community-source failures are not replaced with a global fallback
market.
