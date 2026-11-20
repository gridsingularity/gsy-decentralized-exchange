# GSY DEX Matching Engine

## Role in the System

`gsy-matching-engine` performs off-chain market matching and submits settlement batches
to `TradeSettlement`.

## Trigger Model

The trigger depends on the matching algorithm.

### Pay-as-bid

In Web3 mode, the engine polls block numbers every `2s` and matches the open
orders on block buckets: every `4` blocks by default, or every
`MATCHING_ENGINE_BLOCK_INTERVAL` blocks.

### Pay-as-clear

Each market is cleared a fixed number of times while it is open, at times
derived from the opening and closing time of that market:

- `PAY_AS_CLEAR_CLEARINGS_PER_MARKET` (default `1`): the open window is
  divided into this many equal parts and the market is cleared at the end of
  each part.
- `PAY_AS_CLEAR_FINAL_CLEARING_LEAD_SECONDS` (default `20`): the last clearing
  takes place this long before the closing time, so that its settlement is
  processed while the market still accepts it. `TradeSettlement` rejects
  orders of a closed market.

With the defaults a market is cleared once, 20 seconds before it closes, so it
has one clearing result and one clearing price. Orders placed in the last 20
seconds are not cleared.

| Market window | Clearings per market | Clearing times after opening |
|---|---|---|
| 15 min | 1 | 14:40 |
| 15 min | 3 | 5:00, 10:00, 14:40 |
| 120 min | 1 | 119:40 |
| 120 min | 3 | 40:00, 80:00, 119:40 |

No clearing takes place inside the lead time: with so many clearings that
several would fall there, they are merged into the one at the lead time. A
market that is open for no longer than the lead time is not cleared.

The engine checks the wall clock every second and reads the open orders every
`5s` to find the markets they belong to; the window of each market comes from
off-chain storage. Consequently:

- A market without open orders is not cleared and produces no clearing result.
- The first orders of a market are noticed up to `5s` after they are stored, so
  a clearing of a market that had no orders before can run up to `5s` late.
- A clearing whose orders cannot be read is retried every second until the
  market closes. A clearing whose `settleBatch` transaction fails is not
  repeated.
- Clearing times that pass while the engine is down are not made up for.
- `MATCHING_ENGINE_BLOCK_INTERVAL` is not used.

The schedule is enforced by the engine. The contracts do not limit how many
times a market is settled.

## Matching Pipeline

1. Fetch open orders from off-chain storage API (`/orders`).
2. Convert DB schema into canonical matching primitives.
3. Partition orders by `(market_id, time_slot)`.
4. Run the configured matching algorithm independently for each partition,
   with preference phase first.
5. Build the `settleBatch` payload and submit it with the matching engine signer.
6. The off-chain-storage listener creates `Matched` trades from confirmed
   `TradeSettled` events. The matching engine does not write trade records or
   publish trade-status notifications before confirmation.

A failed or dropped transaction creates no trade record. `Settled` is reserved
for the time the trade has been processed in the Settlement market, not the
on-chain match confirmation. This transition is not yet implemented.

## Matching Algorithms

`MATCHING_ALGORITHM` selects the standard-market algorithm:

- `pay_as_bid` (default): each accepted standard match settles at the bid rate.
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
2. **Standard phase**: remaining bids/offers run through the configured
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
  - `MATCHING_ALGORITHM` (`pay_as_bid` by default; `pay_as_clear` supported)
  - `PAY_AS_CLEAR_PRICING` (`max_offer` by default; `min_bid` and `midpoint` supported)
  - `MATCHING_ENGINE_BLOCK_INTERVAL` (optional; `pay_as_bid` only, defaults
    to `4`)
  - `PAY_AS_CLEAR_CLEARINGS_PER_MARKET` (optional; `pay_as_clear` only,
    defaults to `1`, must be greater than `0`)
  - `PAY_AS_CLEAR_FINAL_CLEARING_LEAD_SECONDS` (optional; `pay_as_clear`
    only, defaults to `20`)

An invalid value of a `PAY_AS_CLEAR_…` variable stops the engine at startup;
an empty value takes the default.
