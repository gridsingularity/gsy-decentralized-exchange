# GSY DEX Market Orchestrator

## Purpose

`gsy-market-orchestrator` creates markets on-chain through `MarketController`.
It only creates markets; a market opens and closes by its own opening and
closing time, so there is no open/close step.

## Core Behavior

1. Wait until orchestrator signer has `ORCHESTRATOR_ROLE`.
2. Run periodic ticks (`tick_interval_seconds`).
3. Fetch the current communities from off-chain storage through the configured
   HTTP or EWDS transport.
4. For each market type, select the delivery slots whose market opens in
   `(now, now + LOOK_AHEAD_HOURS]`, plus the current delivery slot
   (`deliveryStart <= now < deliveryEnd`) so its markets exist right after a
   (re)start even if they have already opened. Markets of later slots whose
   opening time has passed are not created.
5. For each selected slot and community:
   - Compute the deterministic `marketId`.
   - Ask `MarketController.marketExists`; the chain is the single source of
     truth, the orchestrator keeps no state between ticks.
   - Build the full record for missing markets: community UUID (as `bytes16`),
     opening/closing time from the market-type offsets, delivery start and end
     (`+ TIME_SLOT_SEC`), market type and matching algorithm.
6. Send the missing markets through `createMarkets` in transactions of at most
   `MARKET_CREATION_BATCH_SIZE` markets. The contract skips markets that
   already exist, so a race with a pending transaction does not revert a batch.

A community whose ID is neither a UUID nor a 16-byte hex value is skipped and
logged.

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
- `LOOK_AHEAD_HOURS`: how far ahead opening times are considered
- `MARKET_CREATION_BATCH_SIZE` (default `50`, must be greater than `0`)
- `MATCHING_ALGORITHM` (default `pay_as_bid`): written into every market
  record; set it to the value the matching engine uses. It is held per
  market type in `MARKET_RULES` and is the same for all types today.
- `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
- `OFFCHAIN_STORAGE_URL`
- `EWDS_MARKET_ORCHESTRATOR_CLIENT_ID`
- Market window offsets (via global constants/env:
  `SPOT_MARKET_OPEN_OFFSET_MIN`, `SPOT_MARKET_CLOSE_OFFSET_MIN`, and the
  `FLEX_…` / `SETTLEMENT_…` equivalents, in minutes relative to delivery
  start)

## Failure Handling

Each tick is isolated. Community-source, RPC, and transaction errors are logged
and the next tick continues, so transient failures do not stop orchestration
permanently. Markets of a failed batch are still missing on-chain and are
sent again on the next tick.

If the orchestrator is down when a market of a later delivery slot opens, that
market is not created; only the current delivery slot is covered on restart. Community-source failures are not replaced with a global fallback
market.
