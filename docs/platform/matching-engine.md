# GSY DEX Matching Engine

## Role in the System

`gsy-matching-engine` performs off-chain market matching and submits match batches
to `TradeMatching`.

## Trigger Model

Every market is cleared a fixed number of times while it is open, at times
derived from its own opening and closing time, whatever its matching
algorithm. The algorithm of the market decides only how its order books are
matched (see [Matching Algorithms](#matching-algorithms)).

- `CLEARINGS_PER_MARKET` (default `1`): the open window is divided into this
  many equal parts and the market is cleared at the end of each part.
- `FINAL_CLEARING_LEAD_SECONDS` (default `20`): the last clearing takes place
  this long before the closing time, so that its matches are recorded while
  the market still accepts them. `TradeMatching` rejects orders of a closed
  market.

With the defaults a market is cleared once, 20 seconds before it closes, so it
has one clearing result. Orders placed in the last 20 seconds are not cleared.

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
`5s` to find the markets they belong to. The record of each market, with its
window and matching algorithm, comes from off-chain storage. Consequently:

- A market without open orders is not cleared and produces no clearing result.
- The first orders of a market are noticed up to `5s` after they are stored, so
  a clearing of a market that had no orders before can run up to `5s` late.
- A market whose record cannot be read is not cleared until a later discovery
  reads it.
- A market stays tracked while it has open orders, also after it closed, so
  its record is read only once. A closed market is never cleared.
- A clearing whose orders cannot be read is retried every second until the
  market closes. A clearing whose `submitMatches` transaction fails is not
  repeated.
- Clearing times that pass while the engine is down are not made up for.

The schedule is enforced by the engine. The contracts do not limit how many
times a market is cleared.

## Matching Pipeline

1. Fetch open orders from off-chain storage API (`/orders`).
2. Convert DB schema into canonical matching primitives.
3. Keep the orders of the markets that are due for clearing.
4. Partition orders by `(market_id, time_slot)`.
5. Group the partitions by the matching algorithm of their market record.
6. Run each group's algorithm independently for each partition, with
   preference phase first.
7. Build one `submitMatches` payload for all due markets and submit it with
   the matching engine signer.
8. The off-chain-storage listener creates `Matched` trades from confirmed
   `TradeMatched` events. The matching engine does not write trade records or
   publish trade-status notifications before confirmation.

Known non-Submitted orders are ignored before full conversion. A malformed
Submitted order (or an unknown status) excludes its entire market/time-slot
book, including otherwise valid orders; an unreadable slot excludes the whole
market. No matches or clearing results are emitted for excluded books, while
other books can proceed. An unidentifiable market or failed storage response
aborts the cycle. This isolation applies to orders decoded
from the response, not to unreadable MongoDB records or response envelopes.

A failed or dropped transaction creates no trade record. `Settled` is reserved
for the time the trade has been processed in the Settlement market, not the
on-chain match confirmation. This transition is not yet implemented.

## Matching Algorithms

The `matchingAlgorithm` of each market record selects the market's
standard-market algorithm, so one engine instance matches markets of different
algorithms side by side. The [market orchestrator](market-orchestrator.md)
creates Spot markets with `pay_as_clear`, and Flex and Settlement markets with
`pay_as_bid`. The engine reads the record from off-chain storage, which
indexes it from `NewMarketCreated`. `TradeMatching` reads the same algorithm
from `MarketController` and reverts a batch of a pay-as-clear market whose
standard matches do not share one price (`NonUniformClearingPrice`).
Markets with `amm` are not cleared, because the engine does not implement it;
their orders stay open.


- `pay_as_bid` (default): each accepted standard match is priced at the bid rate.
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
the bid's effective rate and is priced at the bid's effective rate (pay-as-bid),
even when the standard phase uses pay-as-clear. Preferred rates supersede the
normal price limits and may lie outside them.

For example, a preferred bid of 15 and offer of 12 are matched at 15. If the bid's
effective rate is below the offer's, no preferred match is possible.
The orders remain eligible for standard matching at their normal rates,
as do orders whose preferred partners are missing or incompatible. A preference
is not an exclusive trading restriction. Allocation follows input order, not
guaranteed insertion order; there is no explicit preference price/time priority.

Each result records `MatchType::Preferred` or `MatchType::Standard`. Residuals
inherit the original metadata and receive new IDs. Subsequent fills consume
those exact IDs, including across the preference/standard boundary. Submitting
the matches atomically registers residuals on-chain, so they can trade in the
same batch or a later cycle. The listener processes placement and match events
in chain order to preserve their indexed lifecycle.

## Contract Interaction

Orders and preferences are compared using exact off-chain owner IDs. The engine
derives keyed actor hashes locally only when building contract arguments, including
preferred partners. It makes no ID-mapping requests. Known-incomplete storage
projections return an error, so the cycle does not match a partial book.

Before submission, engine checks `hasRole(OPERATOR_ROLE, signer)` on the `TradeMatching` contract.
`submitMatches` transaction success is validated via receipt status.
The contract validates the submitted match type, counterparties and price;
it does not prove that the operator processed the preference phase first or
selected the best available counterpart. See
[matching invariants](smart-contracts.md#matching-invariants).

## Key Config

- CLI: `web3 <offchain_storage_host> <offchain_storage_port> <node_host> <node_port>`
- Env:
  - `OFFCHAIN_STORAGE_URL` (otherwise the CLI storage host/port)
  - `OFFCHAIN_STORAGE_TRANSPORT` (`http` or `ewds`)
  - `TRADE_MATCHING_ADDRESS`
  - `ACTOR_ID_HASH_KEY` (shared actor pseudonym key, required at startup)
  - `MATCHING_ENGINE_PRIVATE_KEY`
  - `PAY_AS_CLEAR_PRICING` (`max_offer` by default; `min_bid` and `midpoint`
    supported; used by every pay-as-clear market)
  - `CLEARINGS_PER_MARKET` (optional; defaults to `1`, must be greater than
    `0`)
  - `FINAL_CLEARING_LEAD_SECONDS` (optional; defaults to `20`)

Any market can use pay-as-clear, so an invalid value of one of these three
variables stops the engine at startup. An empty clearing variable takes its
default. `MATCHING_ALGORITHM`, `MATCHING_ENGINE_BLOCK_INTERVAL`,
`PAY_AS_CLEAR_CLEARINGS_PER_MARKET` and
`PAY_AS_CLEAR_FINAL_CLEARING_LEAD_SECONDS` are no longer read.
