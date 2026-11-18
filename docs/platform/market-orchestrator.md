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
4. For each market type, select the delivery slots whose market has not
   closed (`now < closingTime`) and opens by `now + LOOK_AHEAD_HOURS`.
   Markets are normally created before they open; markets that have already
   opened are included too, so a new community or a (re)start after downtime
   still gets every market it can trade in. Closed markets are never created.
5. For each selected slot and community, build a `MarketKey` from the keyed
   hash of the exact community ID (`bytes16`), market type and delivery start.
6. Call `MarketController.getMarketIds(MarketKey[])` in batches of at most 500
   tuples. Results preserve request order: the existing chain ID or zero when
   absent. The chain is the single source of truth; no state is kept between ticks.
7. Only for an absent tuple, generate a random UUIDv4 `marketId` once for the
   candidate and build its record: community hash, opening/closing time from
   the market-type offsets, delivery start/end (`+ TIME_SLOT_SEC`), market type
   and matching algorithm. Send candidates through `createMarkets` in batches
   of at most `MARKET_CREATION_BATCH_SIZE`. Duplicate tuples, including races
   with pending transactions, are skipped; an ID collision with a different
   tuple is rejected.

Community IDs are exact, nonempty, unpadded opaque strings, including UUID-looking
values; case is preserved. Empty or padded IDs are skipped and logged without
blocking valid communities. No UUID parsing or case normalization is applied.

Communities are fetched on every tick so pilot-site updates are observed without
restarting the orchestrator. A tick is skipped when no communities exist.

## Market Identity

Market IDs are random UUIDv4 values, not hashes derived from community and time.
The on-chain uniqueness key is `(communityId, marketType, deliveryStartTime)`,
where `communityId` is the `bytes16` encoding of `derive_onchain_id` on the exact
off-chain community string, using the shared `ACTOR_ID_HASH_KEY`.

Callers look up the existing tuple rather than reconstructing its market ID.
Once created, that ID is reused across ticks and restarts. Storage represents
it as lowercase `0x` bytes16 hex; EWDS market payloads format the same bytes as
a UUID and preserve the original opaque community ID.

## Configurable Parameters

- `EVM_NODE_URL`
- `MARKET_CONTROLLER_ADDRESS`
- `ORCHESTRATOR_SIGNER_PRIVATE_KEY`
- `ACTOR_ID_HASH_KEY`: shared keyed community/actor hash configuration
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
permanently. The next tick looks up each tuple again: confirmed markets reuse
their chain IDs, and only tuples still absent receive new candidates.

If the orchestrator is down when a market opens, the market is created on the
first tick after the restart, as long as it has not closed yet. Markets that
closed during the downtime are not created. Community-source failures are not
replaced with a global fallback market.
