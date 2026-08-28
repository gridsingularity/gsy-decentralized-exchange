# GSY DEX Matching Engine

## Role in the System

`gsy-matching-engine` performs off-chain market matching and submits settlement batches
to `TradeSettlement`.

## Trigger Model

In Web3 mode, the engine polls block numbers and triggers matching on block buckets:

- `pay_as_bid`: every `4` blocks by default
- `pay_as_clear`: every `64` blocks by default
- `MATCHING_ENGINE_BLOCK_INTERVAL`: optional positive-integer override
- Poll interval: `2s`

The longer `pay_as_clear` interval allows the engine to aggregate the order
book before calculating one clearing point. Clearing a partially submitted
book in multiple cycles would create multiple clearing prices and would no
longer represent one uniform-price auction.

## Matching Pipeline

1. Fetch open orders from off-chain storage API (`/orders`).
2. Convert DB schema into canonical matching primitives.
3. Partition orders by `(market_id, time_slot)`.
4. Run the configured matching algorithm independently for each partition,
   with preference phase first.
5. Build EVM tuple payload for `settleBatch`.
6. Submit transaction with matching engine signer.

## Matching Algorithms

`MATCHING_ALGORITHM` selects the standard-market algorithm:

- `pay_as_bid` (default): each accepted standard match settles at the bid rate.
- `pay_as_clear`: bids are sorted by descending energy_rate and offers by ascending energy_rate, then the
  engine walks their cumulative energy tranches until the next bid price is
  lower than the next offer price. The accepted cumulative quantity is the
  clearing volume. All accepted standard matches settle at the marginal
  accepted offer rate; bids and offers beyond the crossing remain open.

The existing bilateral preference phase runs before either standard-market
algorithm. Preference matches retain their negotiated preferred rate; the
uniform clearing price applies to the remaining merit-order book.
The pay-as-clear E2E feature covers both the standalone merit-order auction
and a combined clearing cycle containing a preferred bilateral trade plus
standard bids and offers.

Both algorithms operate on one market and delivery slot at a time. Orders from
different markets or slots cannot match each other, and each pay-as-clear
partition calculates its own clearing volume and price.

## Preference Matching Behavior

The matching algorithm executes:

1. **Preference phase**: bilateral partner constraints are applied first.
2. **Standard phase**: remaining bids/offers run through the configured
   `pay_as_bid` or `pay_as_clear` algorithm.

Residual energy from partially matched orders is carried as new residual entries for next phase/cycle logic.

## Contract Interaction

Before submission, engine checks `hasRole(OPERATOR_ROLE, signer)` on settlement contract.
`settleBatch` transaction success is validated via receipt status.

## Key Config

- CLI: `web3 <offchain_storage_host> <offchain_storage_port> <node_host> <node_port>`
- Env:
  - `TRADE_SETTLEMENT_ADDRESS`
  - `MATCHING_ENGINE_PRIVATE_KEY`
  - `MATCHING_ALGORITHM` (`pay_as_bid` by default; `pay_as_clear` supported)
  - `MATCHING_ENGINE_BLOCK_INTERVAL` (optional; defaults to `4` for
    `pay_as_bid` and `64` for `pay_as_clear`)
