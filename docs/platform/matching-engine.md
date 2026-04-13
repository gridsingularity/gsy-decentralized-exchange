# GSY DEX Matching Engine

## Role in the System

`gsy-matching-engine` performs off-chain market matching and submits settlement batches
to `TradeSettlement`.

## Trigger Model

In Web3 mode, the engine polls block numbers and triggers matching on block buckets:

- `MATCH_PER_NR_BLOCKS = 4`
- Poll interval: `2s`

This avoids over-triggering while preserving deterministic cadence.

## Matching Pipeline

1. Fetch open orders from Orderbook API (`/orders`).
2. Convert DB schema into canonical matching primitives.
3. Run pay-as-bid algorithm with preference phase first.
4. Build EVM tuple payload for `settleBatch`.
5. Submit transaction with matching engine signer.

## Preference Matching Behavior

The matching algorithm executes:

1. **Preference phase**: bilateral partner constraints are applied first.
2. **Standard phase**: remaining bids/offers run through normal pay-as-bid matching.

Residual energy from partially matched orders is carried as new residual entries for next phase/cycle logic.

## Contract Interaction

Before submission, engine checks `hasRole(OPERATOR_ROLE, signer)` on settlement contract.
`settleBatch` transaction success is validated via receipt status.

## Key Config

- CLI: `web3 <orderbook_host> <orderbook_port> <node_host> <node_port>`
- Env:
  - `TRADE_SETTLEMENT_ADDRESS`
  - `MATCHING_ENGINE_PRIVATE_KEY`
