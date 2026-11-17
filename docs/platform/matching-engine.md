# GSY DEX Matching Engine

## Role in the System

`gsy-matching-engine` performs off-chain market matching and submits settlement batches
to `TradeSettlement`.

## Trigger Model

In Web3 mode, the engine polls block numbers every `2s`. Each market is matched
on the block interval of its matching algorithm:

- `pay_as_bid`: every `4` blocks by default
  (`MATCHING_ENGINE_PAY_AS_BID_BLOCK_INTERVAL` overrides it)
- `pay_as_clear`: every `64` blocks by default
  (`MATCHING_ENGINE_PAY_AS_CLEAR_BLOCK_INTERVAL` overrides it)

An algorithm is due when one of the blocks mined since the previous poll is a
multiple of its interval, so a boundary is not missed when several blocks are
mined between two polls. If no algorithm is due, the engine does not fetch any
orders. On a block that is a multiple of both intervals, markets of both
algorithms are matched in the same cycle.

The longer `pay_as_clear` interval allows the engine to aggregate the order
book before calculating one clearing point. Clearing a partially submitted
book in multiple cycles would create multiple clearing prices and would no
longer represent one uniform-price auction.

## Matching Pipeline

1. Fetch open orders from off-chain storage API (`/orders`).
2. Convert DB schema into canonical matching primitives.
3. Partition orders by `(market_id, time_slot)`.
4. Look up the market record of each partition (`GET /market` or
   `markets.query`) and group the partitions by the record's
   `matchingAlgorithm`. Market records are cached, since a market never changes
   after `MarketController` created it. Orders of a market without a record,
   or with the unimplemented `amm` algorithm, are skipped with a warning; they
   stay open and are retried in the next cycle.
5. Run each due group's matching algorithm independently for each partition,
   with preference phase first.
6. Build one `settleBatch` payload for all groups and submit it with the
   matching engine signer.
7. The off-chain-storage listener creates `Matched` trades from confirmed
   `TradeSettled` events. The matching engine does not write trade records or
   publish trade-status notifications before confirmation.

A failed or dropped transaction creates no trade record. `Settled` is reserved
for the time the trade has been processed in the Settlement market, not the
on-chain match confirmation. This transition is not yet implemented.

## Matching Algorithms

The `matchingAlgorithm` of each market record selects the standard-market
algorithm for that market. The market orchestrator writes it per market type:
Spot markets use `pay_as_clear`, Flex and Settlement markets `pay_as_bid`.

- `pay_as_bid`: each accepted standard match settles at the bid rate.
- `pay_as_clear`: bids are sorted by descending energy_rate and offers by ascending energy_rate, then the
  engine walks their cumulative energy tranches until the next bid price is
  lower than the next offer price. The accepted cumulative quantity is the
  clearing volume. Accepted standard matches share a price selected by
  `PAY_AS_CLEAR_PRICING` (`max_offer`, `min_bid`, or `midpoint`, default
  `max_offer`). Supply exhaustion with price-eligible demand remaining uses the
  minimum accepted bid; demand exhaustion with price-eligible supply remaining
  uses the maximum accepted offer. Simultaneous exhaustion or no price-eligible
  remainder uses the minimum accepted bid. Price crossings use the configured
  policy. Equal rates count as price-eligible. Orders beyond the clearing volume
  remain open.

The preference phase runs before either standard-market
algorithm. Preference matches use pay-as-bid at the buyer's effective rate; the
uniform clearing price applies to the remaining merit-order book.
The pay-as-clear E2E feature covers both the standalone merit-order auction
and a combined clearing cycle containing a preferred bilateral trade plus
standard bids and offers.

Both algorithms operate on one market and delivery slot at a time. Orders from
different markets or slots cannot match each other, and each pay-as-clear
partition calculates its own clearing volume and price.

## Preference Matching Behavior

The matching algorithm executes:

1. **Preference phase**: eligible partner preferences are matched first.
2. **Standard phase**: remaining bids/offers run through the market's
   `pay_as_bid` or `pay_as_clear` algorithm.

Self-trades (equal `created_by`) are rejected in both phases and on-chain.
Both bids and offers use `requirements.trading_partner_id`, compared with the
counterparty's `created_by`. At least one side must name a preferred partner.
One-sided preferences are valid, but every declared preference must identify
the actual counterparty: when both sides declare partners, they must be reciprocal.

For each side, the effective rate is its non-zero `preferred_energy_rate`, or
its normal `energy_rate` when the preferred rate is absent (zero on-chain).
A preferred match requires the offer's effective rate to be no higher than
the bid's effective rate and settles at the bid's effective rate (pay-as-bid),
even when the standard phase uses pay-as-clear. Preferred rates supersede the
normal price limits and may lie outside them.

For example, a preferred bid of 15 and offer of 12 settle at 15. If the bid's
effective rate is below the offer's, no preferred match is possible.
The orders remain eligible for standard matching at their normal rates,
as do orders whose preferred partners are missing or incompatible. A preference
is not an exclusive trading restriction. Allocation follows input order, not
guaranteed insertion order; there is no explicit preference price/time priority.

Each result records `MatchType::Preferred` or `MatchType::Standard`. Residuals
inherit the original metadata and receive new IDs. Subsequent fills consume
those exact IDs, including across the preference/standard boundary. Settlement
atomically registers residuals on-chain, so they can trade in the same batch or
a later cycle. The listener processes placement and settlement events in chain
order to preserve their indexed lifecycle.

## Contract Interaction

Before submission, engine checks `hasRole(OPERATOR_ROLE, signer)` on settlement contract.
`settleBatch` transaction success is validated via receipt status.
The contract validates the submitted match type, counterparties and price;
it does not prove that the operator processed the preference phase first or
selected the best available counterpart. See
[settlement invariants](smart-contracts.md#settlement-invariants).

## Key Config

- CLI: `web3 <offchain_storage_host> <offchain_storage_port> <node_host> <node_port>`
- Env:
  - `OFFCHAIN_STORAGE_URL` (otherwise the CLI storage host/port)
  - `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
  - `TRADE_SETTLEMENT_ADDRESS`
  - `MATCHING_ENGINE_PRIVATE_KEY`
  - `PAY_AS_CLEAR_PRICING` (`max_offer` by default; `min_bid` and `midpoint`
    supported). Applies to every pay-as-clear market and is validated at
    startup, so an invalid value stops the engine.
  - `MATCHING_ENGINE_PAY_AS_BID_BLOCK_INTERVAL` and
    `MATCHING_ENGINE_PAY_AS_CLEAR_BLOCK_INTERVAL` (optional positive integers;
    defaults `4` and `64`)
- The algorithm is not configured on the engine: it comes from each market
  record.
