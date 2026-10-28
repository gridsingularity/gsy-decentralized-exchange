# GSY DEX Execution Engine

## Role in the System

`gsy-execution-engine` computes imbalance penalties after trade settlement and
records delivery verification in `TradeSettlement`.

## Execution Cycle

Each cycle:

1. Select the current target timeslot or an older slot still within its rollover grace period.
2. Fetch trades plus measurement points/timeseries for that window from off-chain storage.
3. Compute verdicts for `Matched` or `Penalized` trades with a usable measurement,
   excluding trades whose on-chain verification is already complete.
4. Submit `completeDeliveryVerification` and wait for a successful receipt.
   Penalties (if any) and completion are recorded atomically. Fully delivered
   trades also complete, without a separate penalty transaction.

## Penalty Computation

Current penalty calculator logic:

- `delta = measured_energy - traded_energy`
- `delta > 0`: penalize buyer
- `delta < 0`: penalize seller
- Penalty scaled with `NODE_FLOAT_SCALING_FACTOR` (`10000`)

Measurement identities are derived locally with the ID service's shared
Blake2b-16 helper, without querying or writing ID mappings. This is the same
identity used in `OrderParams.createdBy` and the indexed trade parties. Owner
IDs and service wallets are not measurement identities. The penalty policy is
unchanged: the buyer's measurement is preferred, with the seller's as a
fallback. This does not independently verify both sides. Multiple measurements
for a facility overwrite each other, and allocation across multiple trades is not
handled. These policy limitations need separate agreement; `Executed` records
completion under this current policy, not proof that both meters were checked.
Missing/unmapped or non-finite measurements produce no verdict, not a zero-penalty
success. HTTP and EWDS use the same calculation and on-chain completion path.

## Duplicate Submission Protection

Before submitting, the engine checks `settledTrades(tradeId).verified` and skips
completed trades. The contract also ignores repeated verdicts, so receipt loss
or concurrent submissions cannot duplicate penalties or completion events.
Matching pre-recorded penalties are reused; conflicting amounts reject the
transaction. A successful receipt is required before reporting progress.

## Timeslot Rollover

Trades or measurements can arrive after the configured target delivery slot
advances, with either HTTP or EWDS. The engine keeps each outgoing slot eligible
for `EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` after its scheduled target rollover
(including `EXECUTION_ENGINE_OFFSET_MIN`). The default is 900 seconds; set `0`
to disable retention. At the deadline the slot is no longer selected, but an
already-running cycle is allowed to finish.

The scheduler alternates the current target with retained slots, visiting the
retained slots oldest-first in rotation. A newly advanced target is selected
first. This prevents an older slot's retries from monopolizing polling cycles.
Execution and verification submissions remain serial; the polling interval and
transport latency still determine how often each slot can be checked.

Empty results, failures, and successful submissions do not remove a retained
slot early: additional records can still arrive. The existing on-chain verification
check prevents resubmission for trades already processed. The eligible window
is recalculated from time, including after restart or skipped polling intervals;
neither restarts nor retries extend the deadline. Records arriving beyond that
window require separate historical processing, which is not implemented here.

`EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` replaces the cycle-count setting
`EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT`; update existing environment overrides.
The old setting is ignored, not converted to seconds. Invalid grace-period
values fail startup rather than silently falling back to the default.

## Contract Interaction

- Required role: `EXECUTION_ENGINE_ROLE`
- Submission call: `completeDeliveryVerification(tuple[])`
- Success criteria: transaction receipt status is `1`

The engine does not write lifecycle statuses through HTTP/EWDS. The off-chain
storage listener indexes `DeliveryVerified` as `Executed` for the two consumed
orders and for penalty-free trades. A penalized trade remains `Penalized`.
Residual order IDs remain independent.

## Focused Lifecycle Test

The real-contract test is opt-in because it needs compiled Hardhat artifacts,
Anvil on `PATH`, and MongoDB (`DATABASE_*`). After compiling `gsy-contracts` with
`npx hardhat compile`, run from the workspace root:

```bash
OFFCHAIN_STORAGE_TRANSPORT=http cargo test --locked -p gsy-offchain-storage \
  --test api delivery_verification -- --ignored --test-threads=1
```

`GSY_CONTRACT_ARTIFACTS` can point to artifacts compiled in another container.
The test deploys isolated proxies and exercises missing/late measurements,
zero/non-zero penalties, replay safety, timestamps, and independent residuals.

## Key Config

- CLI: `web3 <offchain_host> <offchain_port> <node_host> <node_port> <polling_interval> <market_duration> <penalty_rate>`
- Env:
  - `TRADE_SETTLEMENT_ADDRESS`
  - `EXECUTION_ENGINE_PRIVATE_KEY`
  - `EXECUTION_ENGINE_OFFSET_MIN`
  - `EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` (default `900`; set `0` to disable)
