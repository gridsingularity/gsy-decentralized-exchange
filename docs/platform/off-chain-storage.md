# GSY DEX Off-Chain Storage

## Purpose

`gsy-offchain-storage` is the off-chain state API and persistence layer.
It stores indexed on-chain events plus ontology-aligned market and profile data.

Backend: MongoDB (version pinned in the Compose files). Reading communities, sites
and facilities needs MongoDB 5.2 or later, for the `$top` accumulator.

## Event Indexing Path

1. `gsy-ethers-listener` subscribes to:
   - `OrderPlaced`
   - `OrderCancelled`
   - `TradeMatched`
   - `PenaltyRecorded`
   - `DeliveryVerified`
   - `NewMarketCreated`
   - `MarketClearing`
2. `OffchainStorageEvmHandler` maps event payloads into DB schemas.
3. `gsy-offchain-storage` updates order/trade records and exposes them via REST APIs.

`OrderPlaced` contains the optional order requirements (bids and offers) and
offer attributes, so the indexed order is complete without a follow-up
`/orders` write. `POST /orders` and `POST /orders-normalized` remain available as
insert-only imports, but neither is used by the E2E scenarios or production
order lifecycle. Orders are submitted through `placeOrder` (directly or through
the community client's EWDS subscriber), then indexed by the EVM listener.
Imports validate the whole payload and canonicalize UUIDv4 order and market IDs
to lowercase UUID text. `createdBy` and preferred partners contain
original off-chain owner IDs, preserved exactly. These routes do not submit orders.
Invalid identifiers return 400. Duplicate canonical IDs within the batch or
already in storage return 409 before writes. Unique indexes also prevent
concurrent imports from replacing records. A later `OrderPlaced` replaces an
import with authoritative chain data, and listener upserts remain replayable.
Import batches are not transactional: a concurrent conflict or database failure
can leave earlier inserts in place. Imports do not publish status notifications.
The POST endpoints still accept caller-supplied statuses for new IDs; their
ownership and access policy remain to be clarified. Posting trades leaves referenced orders
unchanged; match updates are handled by the EVM listener.
Access control for the order POST endpoints remains a separate follow-up.
Until then, listener ownership is the normal lifecycle path, not an
enforced access-control boundary for those legacy write endpoints.

`NewMarketCreated` carries the full market record. The handler converts it to
`MarketSchema` (`market_id` as lowercase UUIDv4, `community_id` as
the exact off-chain string, times as BSON dates). It reverse-resolves
the event's community hash directly through the DB ID mapping registered by
topology writes, without calling the IDs API or interpreting the hash as a UUID.
A missing mapping retains the original event in the durable inbox described below.
It delays dependent projections, not reception of later blocks or unrelated markets.
The handler upserts by `market_id`, so a replayed event is harmless. The chain is
the source of markets; no service posts them any more.

## HTTP API Surface

### Health
- `/health_check` (`GET`)

### Order Book Storage (D3.2 section 5.4)
- `/orders-normalized` (`POST`)
- `/orders` (`GET`, `POST`)
- `/flexibility-orders` (`GET`, `POST`)
- `/tariffs` (`GET`, `POST`)

### Trades Storage (D3.2 section 5.3)
- `/trades-normalized` (`POST`)
- `/trades` (`GET`, `POST`); `GET` follows the `trades.query` rules: `market_id` alone is enough, otherwise both `start_time` and `end_time` are required and span at most 1 day (`400` otherwise)
- `/market` (`GET`) compatibility adapter for EVM JSON callers
- `/communities` (`GET`, `POST`) for versioned communities (see Grid Topology)
- `/markets` (`GET`, `POST`) for ontology-aligned market-opening records;
  records come from `NewMarketCreated`, `POST` remains for manual use
- `/clearing-results` (`GET`, `POST`)
- `/market-roles` (`GET`, `POST`)

### Measurements Storage (D3.2 section 5.2)
- `/measurements` (`GET`, `POST`)
- `/forecasts` (`GET`, `POST`)
- `/measurement-points` (`GET`, `POST`) for ontology-aligned measurement metadata
- `/timeseries` (`GET`, `POST`) for ontology-aligned values

### Grid Topology and Market Storage (D3.2 section 5.1)
- `/assets` (`GET`, `POST`)
- `/pilot-sites` (`GET`, `POST`)
- `/communities` (`GET`, `POST`)
- `/sites` (`GET`, `POST`)
- `/facilities` (`GET`, `POST`)
- `/facilities/validate` (`POST`) for read-only bulk facility/market validation

Community writes through HTTP and EWDS accept exact, nonempty, unpadded opaque IDs,
including UUID-looking strings. `GET /communities` and `communities.query` preserve
their spelling and case. Empty or padded IDs return HTTP 400 or cause the EWDS event to be dropped
as invalid; they are not retried as storage failures.

`GET /market`, `GET /markets`, `markets.query`, `POST /markets`, and
`POST /clearing-results` normalize market UUIDv4s to lowercase UUID text.
Only hyphenated UUIDv4 values are accepted (either letter case). Hex, bare,
padded, nil and non-v4 IDs return HTTP 400 or EWDS `invalid_request`.
Market writes and community filters preserve exact opaque community IDs.
HTTP and EWDS market reads and events use the same UUID format.
Facility-validation responses still echo each requested pair exactly and report
an invalid market ID as `unknown_market` for that pair only.

`POST /sites` and `/facilities` store a new version of the site or facility
with that `site_id` or `facility_id`, like the EWDS events. Send complete
records: a new version of a site's facility list removes omitted memberships and
can make those facilities ineligible. A name that another valid entry uses
returns 409. These topology writes remain unauthenticated; restricting that
write surface is a separate access-control follow-up.

Facility IDs, owner IDs and site facility references preserve their exact spelling,
including UUID-shaped, bare or `0x` hex strings; they are case-sensitive and are
not interpreted as resolved on-chain IDs.
Empty or padded identities return HTTP 400 or invalidate the entire inbound EWDS
event before any writes. Site facility lists deduplicate identical references only.
Site IDs follow the same actor-ID rule; names and arbitrary asset IDs are unchanged.
These identity rules target fresh deployments with storage and consumers built
from the same revision.

### Facility Participation Validation

`POST /facilities/validate` accepts an array of
`{"facility_id": "facility-a", "market_id": "808f0603-acbe-4696-8d3d-a1cd0cb3f5f4"}` pairs. Facility IDs are
the original off-chain IDs, not owner IDs or generated on-chain IDs. Market IDs
accept UUIDv4; results echo the supplied IDs, with invalid markets reported per pair.
Facility lookup uses the exact off-chain identity, and responses preserve
the requested spelling. Empty or padded facility IDs yield `unknown_facility`
for that pair, without failing the other pairs.

Each pair requires an existing market and facility, the facility's site to
exist and list that facility, and the market's community to exist and list
that site. The lookup uses the latest version of each facility, site and
community; deleted ones count as unknown. An ID mapping alone does not
establish participation. This does not validate market opening times
or wallet permissions. A known-pending market projection returns a retryable
storage error, not an `unknown_market` eligibility rejection.

HTTP 200 returns one `{facility_id, market_id, eligible, reason?}` per pair,
preserving order and duplicates. Ineligible pairs carry a reason:
`unknown_market`, `unknown_facility`, `unknown_community`, `unknown_site`,
`facility_not_listed_in_site` or `site_not_in_community`. Empty requests return
`[]`. Malformed requests return 400; database/deserialization failures return
500 with `code: VALIDATION_UNAVAILABLE`, not negative eligibility results.

EWDS exposes the same validation through `facilities.validate` when
`EWDS_ENABLE_HANDLER=true`. `OffchainStorageClient::validate_facility_markets`
selects HTTP or EWDS via the client's existing transport configuration and
rejects incomplete or inconsistent responses. Neither transport writes orders
or ID mappings. The community client's EWDS order subscriber validates new
orders through this API before resolving IDs and submitting them. Forecast-based
publication performs the same check. The contract separately requires
`COMMUNITY_CLIENT_ROLE` for submission and cancellation; eligibility is enforced
by those trusted services, not by this read-only endpoint or an ID mapping alone.

The `communities`, `sites` and `facilities` collections keep one document per
version of an entry. `POST` stores a new version unless it equals the latest
one, and `GET` returns the latest version of each entry without the deleted
ones; see [Versions and Deletion](ewds-integration.md#versions-and-deletion).
On startup the service drops the old unique indexes on ids and names, sets a
missing `valid_from` to the Unix epoch, and indexes each collection on its id
and `valid_from`. Both validity times are stored as BSON dates.

### ID Mapping
- Trusted services share keyed Blake2b-16 derivation using `ACTOR_ID_HASH_KEY`
  at contract boundaries. ID-service lookup, insertion and reverse lookup preserve
  exact off-chain strings. `/ids` rejects empty/padded inputs with HTTP 400;
  `ids.query` responds with `invalid_request`. Verification and certificates join
  off-chain owner IDs directly, without hashing or ID-service calls. Certificate
  `owner_id` is the original seller owner ID; historical ownership policy remains
  a separate follow-up.

HTTP and EWDS topology writes register mappings before making the topology record
visible: facility ID and owner ID for facilities, `site_id` for sites, and the
exact opaque community ID for communities.
References to other entities do not register those entities. Replays reuse the
mapping; topology updates retain earlier mappings for chain-event lookups.
If mapping persistence fails, the topology write fails and EWDS can retry.
These writes are not transactional: a failed topology write can leave mappings,
but mappings alone never establish eligibility. A mapping made with another key
is rejected, not overwritten. Use a fresh deployment; do not rotate the key in place.
See [actor key provisioning](../setup/contracts.md#actor-id-hashing-key).
- `/ids` (`POST`) — get-or-create offchain↔onchain ID mappings

`POST /ids` and EWDS `ids.query` remain available for current callers. This
increment does not implement API authentication; access control is deferred
to a separate ticket.

Compatibility adapters for EVM JSON callers:

- `/measurements` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/forecasts` (`GET`, `POST`) converts to/from `MeasurementPoint` + `Timeseries`
- `/market` (`GET`) converts compatibility market JSON to/from `Market`

These adapters do not own separate collections. They read and write the same
`markets`, `measurement_points`, and `timeseries` records as the canonical API.
Measurement/forecast ingest and facility filters use the same facility rule.
Typed profile ingestion requires a nonempty, unpadded opaque community ID,
preserved exactly even when UUID-looking. Generated profile IDs escape `%` to `%25`
then `:` to `%3A` within each community/facility component before joining them with
`:` separators, preventing component collisions. Ordinary UUID/plain IDs without
these characters are unchanged; actor IDs and `asset_name`/`datasource_name` are
not rewritten. Empty or padded IDs reject the whole batch before writes
(HTTP 400 or an invalid EWDS measurement event). The community-client adapter
applies the same checks before forwarding points and values.

`GET /measurements` and `GET /forecasts` accept optional `facility_id` and
`community_uuid` filters, individually or together, plus inclusive `start_time`
and `end_time`. Aliases are `facilityId`/`areaUuid`, `communityUuid`, and
`startTime`/`endTime`. Times are RFC 3339; community and facility filters match
exact strings, with no UUID/hex equivalence or case folding.
Invalid identifiers return HTTP 400. `measurements.query` applies the same
filters and aliases, returning `invalid_request` rather than timing out.
Specify each field only once, not together with one of its aliases.

Direct `/measurement-points` writes preserve `asset_name` and `datasource_name`
exactly, including UUID-looking community references. Other labels, missing
datasource metadata and caller-supplied point IDs are preserved.
Unfiltered profile reads still return these records; a community filter selects
only matching community metadata. Shared read conversions do not validate it.

When `EWDS_ENABLE_HANDLER=true`, the same community collection is available
through the `communities.query` request/reply operation.
The off-chain storage then also stores the measurements, facilities, sites and
communities that other systems publish on the EWDS events channel, in
the same collections (see
[Inbound Events](ewds-integration.md#inbound-events)).

## Order Expiry

The EVM listener expires `Submitted` orders when a confirmed block reaches their
market's indexed on-chain `closingTime`. It processes that block's contract events
first, then checks expiry, including blocks with no contract events. Only open
orders in closed markets are updated, including residuals; consumed and cancelled
orders remain unchanged. `status_updated_at` is the scheduled closing time, so
late indexing and repeated blocks preserve the same deadline. Delivery-slot start
does not trigger expiry. Market windows are immutable and cannot be reopened.

The former wall-clock expiry scheduler and `scheduler_interval` setting have
been removed. HTTP and EWDS use the same listener; order-status notifications
are published only when EWDS is enabled. The first new confirmed block after startup
also checks already-indexed markets for expiry. On first start for a chain and
contract set, the listener records the current head + 1 in MongoDB, without
backfilling older events. It advances this cursor after the block's logs are
durably received and projection/expiry processing succeeds. Unresolved identities
or prerequisites remain in the inbox rather than preventing cursor advancement.
After a reconnect or restart it resumes receipt at the first unfinished block
and replays pending projections. Replays preserve lifecycle states and separate clearing logs;
EWDS notifications can repeat. The cursor is not a chain-reorg recovery mechanism.
There is no reindexing tool. See the
[fresh-deployment requirements](../setup/contracts.md#fresh-deployments).

## Data Model Notes

- The listener creates `Matched` trades when the match is included in a block
  indexed by the listener (no confirmation depth).
  `DeliveryVerified` advances penalty-free trades to `Executed`;
  `PenaltyRecorded` sets terminal `Penalized`, which verification does not replace.
  `Settled` means the trade has been processed in the Settlement market.
  This transition is not yet implemented. Trades cannot be `Rejected`.
  This lifecycle targets fresh deployments; no data migration is provided.
- Existing `POST /trades` and `/trades-normalized` endpoints retain insert-only
  import behavior. They do not update orders or publish lifecycle notifications;
  the matching engine does not use them.
  Imports accept hyphenated UUIDv4 for trade, market and bid/offer IDs,
  storing lowercase UUID text. Buyer/seller IDs are original off-chain owner IDs. Absent residual IDs use null. All identifiers
  are validated before writing (400); duplicate canonical trade IDs in the batch
  or storage return 409. Unique indexes guard concurrent duplicates, but a
  concurrent conflict can still leave a batch partially inserted. No transaction
  or migration is introduced. The seed script assigns one random UUIDv4 market
  ID per community/type/slot within its saved dataset; trade, bid, offer and residual
  IDs are random UUIDv4 too, while parties retain their roster names. Queries reuse saved market IDs, and cleanup deletes only the exact
  trade IDs in that dataset, not a prefix-wide selection.
- With `EWDS_ENABLE_HANDLER=true`, indexed match confirmations, penalties and
  penalty-free delivery verifications publish persisted snapshots through `trade.event`.
  Penalized verification publishes only the consumed orders' status updates,
  since `PenaltyRecorded` already published the trade verdict.
  Replays can notify again but preserve stored statuses and timestamps.
  The event's `occurredAt` uses `status_updated_at`, falling back to creation time
  for legacy records; the trade DTO's `timestamp` remains its RFC 3339 creation time.
  HTTP and EWDS queries return the same DTO. HTTP-only operation needs no gateway.
  Notification delivery remains best effort, without a durable outbox or ordering guarantee.
- Order, trade and market IDs are stored as lowercase UUIDv4 strings.
  Market, trade, order and clearing-result queries accept hyphenated UUIDv4
  market IDs in either letter case and normalize them to lowercase. Invalid IDs return HTTP 400 or
  EWDS `invalid_request`. Storage read failures return HTTP 500 or EWDS
  `STORAGE_UNAVAILABLE`, never a partial list. A malformed stored document blocks
  queries that select it until corrected; the error is logged with query context.
- The preferred trading partner of a bid or an offer is stored in its
  requirements (`requirements.trading_partner_id`) as the original off-chain
  partner owner ID; attributes carry no partner.
  The community client checks the partner is a registered owner with a facility
  and derives its keyed hash locally before contract submission. Partner market
  eligibility is unchanged. Topology ingestion registers the mappings; the
  event indexer uses the ID mapping collection to recover the
  owner IDs. The matcher compares those exact off-chain identities and derives
  their hashes locally only when building contract arguments. UUID-shaped owner
  IDs remain opaque identities, not already-encoded on-chain IDs.
- Missing actor mappings or prerequisite order/trade records are retained in
  `evm_pending_events`, including the original log, block timestamp and chain/contract
  stream. The listener checkpoints only after durable receipt. Dependent events
  wait in chain order; unrelated markets continue indexing. Pending events retry
  on each block and survive restart. Once resolved, the listener writes ordinary
  records with off-chain actor IDs and the original event timestamps. Expiry waits
  for pending events in its market. No actor hash is substituted into an off-chain
  identity field. Database failures and invalid lifecycle transitions still fail
  the block. Pending events have no automatic discard limit.
- Order/trade/market/clearing reads fail with HTTP 500 or EWDS `STORAGE_UNAVAILABLE`
  if their market has pending events. Unfiltered reads, and pending events with
  unknown market scope, conservatively block complete-set queries. This prevents
  matching, verification or certificate issuance using a known-incomplete projection.
  Notification publishing remains best effort; this inbox is not a durable outbox.
- `TradeMatched` events mark consumed orders `Filled`, or `PartiallyFilled` when a
  residual ID is emitted. Each residual is a separate `Submitted` order; filling
  it later does not change its original's status. Matching does not mark orders
  `Executed`; the later `DeliveryVerified` event does so for that trade and only
  its two consumed orders, including `PartiallyFilled` originals. Residuals are
  independent. Consumed orders use the verification block timestamp. A penalty-free
  trade uses it too; a Penalized trade keeps its first penalty block timestamp.
  Replays preserve these timestamps. Separately penalized trades remain eligible
  for verification; the execution engine skips already verified trades on-chain.
  Verified, penalty-free trades are eligible for the status-time/GoO query.
- Order placement, cancellation and match indexing use the event block's
  timestamp for `status_updated_at` (exposed as `updatedAt` in HTTP/EWDS DTOs),
  not the indexing time. Status and timestamp are updated together. Residual
  orders have independent status timestamps; their original creation time does
  not change. A trade's `creation_time` is the match block's timestamp,
  not the wall-clock time of indexing or replay.
  Existing records without a status timestamp are not backfilled by this change.
- Trade records include order IDs, residual IDs and selected match parameters.

## Operational Configuration

Key env variables:

- `EVM_NODE_URL`
- `CONTRACT_ORDER_REGISTRY`
- `CONTRACT_TRADE_MATCHING`
- `CONTRACT_MARKET_CONTROLLER`
- `DATABASE_*`
- `EWDS_ENABLE_HANDLER`
- `EWDS_COMMUNITIES_REQUEST_TOPIC`
- `EWDS_EVENT_PUBLISH_FQCN`, `EWDS_EVENT_SUBSCRIBE_FQCN`
- `EWDS_MEASUREMENTS_EVENT_TOPIC`, `EWDS_FACILITY_EVENT_TOPIC`,
  `EWDS_SITE_EVENT_TOPIC`, `EWDS_COMMUNITY_EVENT_TOPIC`
