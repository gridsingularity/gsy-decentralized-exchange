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
  clearing volume. Accepted standard matches share a price selected by
  `PAY_AS_CLEAR_PRICING` (`max_offer`, `min_bid`, or `midpoint`, default
  `max_offer`). Supply exhaustion with demand remaining uses the minimum
  accepted bid; demand exhaustion with supply remaining uses the maximum
  accepted offer. Price crossings and simultaneous exhaustion use the configured
  policy. Orders beyond the clearing volume remain open.

The preference phase runs before either standard-market
algorithm. Preference matches retain their agreed effective rate; the
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
2. **Standard phase**: remaining bids/offers run through the configured
   `pay_as_bid` or `pay_as_clear` algorithm.

Both bids and offers use `requirements.trading_partner_id`, compared with the
counterparty's `created_by`. At least one side must name a preferred partner.
One-sided preferences are valid, but every declared preference must identify
the actual counterparty: when both sides declare partners, they must be reciprocal.

For each side, the effective rate is its non-zero `preferred_energy_rate`, or
its normal `energy_rate` when the preferred rate is absent (zero on-chain).
A preferred match requires equal effective rates and settles at that exact
rate. It is not pay-as-bid pricing or a midpoint calculation. Preferred rates
supersede the normal price limits and may lie outside them.

For example, reciprocal preferred rates of 15 and 12 do not produce a preferred
match. The orders remain eligible for standard matching at their normal rates,
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
  - `TRADE_SETTLEMENT_ADDRESS`
  - `MATCHING_ENGINE_PRIVATE_KEY`
  - `MATCHING_ALGORITHM` (`pay_as_bid` by default; `pay_as_clear` supported)
  - `PAY_AS_CLEAR_PRICING` (`max_offer` by default; `min_bid` and `midpoint` supported)
  - `MATCHING_ENGINE_BLOCK_INTERVAL` (optional; defaults to `4` for
    `pay_as_bid` and `64` for `pay_as_clear`)
