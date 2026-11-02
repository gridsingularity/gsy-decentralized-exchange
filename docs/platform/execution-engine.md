# GSY DEX Execution Engine

## Role in the System

`gsy-execution-engine` computes imbalance penalties after trade settlement and
records delivery verification in `TradeSettlement`.

## Execution Cycle

Each cycle:

1. Select the current target timeslot or an older slot still within its rollover grace period.
2. Fetch trades plus measurement points/timeseries for that delivery window from
   off-chain storage. Resolve each distinct market once through the existing
   HTTP/EWDS market lookup and group trades by its `delivery_start_time`, not
   trade creation or settlement time.
3. Compute facility totals and penalty allocations separately for each slot,
   including `Executed` and `Settled` trades in the baseline. Only `Matched` or
   `Penalized` trades with usable readings for both parties receive verdicts;
   the connector skips trades whose on-chain verification is already complete.
4. Submit `completeDeliveryVerification` and wait for a successful receipt.
   Penalties (if any) and completion are recorded atomically. Fully delivered
   trades also complete, without a separate penalty transaction.

## Penalty Computation

Current penalty calculator logic:

- For each facility and delivery slot, sum all its indexed purchases and sales,
  across markets: `net_traded_energy = purchases - sales`. Measurements are
  signed: consumption is positive and production negative. A facility that buys
  10 kWh and sells 4 kWh is expected to consume 6 kWh net.
- Select only readings whose `time_slot` equals the market's delivery start.
  Readings from another slot never substitute for a missing one.
- No penalty within `abs(net_traded_energy) * tolerance_percent / 100` of the
  signed net baseline, including the boundaries. The default is 5%. A zero-net
  position has zero tolerance; a nonzero meter reading is a deviation.
- Penalize the buyer for either overconsumption or underconsumption.
- Penalize the seller for either overproduction or underproduction.
- Penalty scaled with `NODE_FLOAT_SCALING_FACTOR` (`10000`)
- Allocate that facility's penalty to its trades in proportion to each trade's
  quantity divided by all its purchases plus sales (not the net quantity).
  This includes both buyer and seller roles, residual trades, and trades already
  verified. Round the total once, then allocate integer units by largest remainder;
  ties use trade ID order. Allocations sum exactly to the facility penalty and
  do not depend on the order of query results.

Outside the tolerance band, the existing `penalty_rate` applies to the **full**
deviation, not only the excess beyond the band. For 100 kWh traded with 5%
tolerance, 105 kWh incurs no penalty; 106 kWh incurs a penalty on 6 kWh. Tolerance
is independent of `penalty_rate` (`0.10` means 10% of the deviation).
For purchases of 4 and 6 kWh and consumption of 12 kWh, the 2 kWh deviation
produces a 2,000-unit penalty at rate 0.10, allocated as 800 and 1,200 units.

Measurement identities are derived locally with the ID service's shared
Blake2b-16 helper, without querying or writing ID mappings. This is the same
identity used in `OrderParams.createdBy` and the indexed trade parties. Owner
IDs and service wallets are not measurement identities. Missing/unmapped,
non-finite, or conflicting readings for a facility/slot leave its trades pending;
they do not produce a zero-penalty success. Identical readings are not counted
twice. Multiple distinct readings are not summed or arbitrarily selected: they
need an unambiguous net measurement before verification. Both parties' readings
are required before atomic completion.
HTTP and EWDS normalize direction-labelled export magnitudes to negative energy;
compatibility profiles retain their original signed values, including when a
point's direction has changed in a later slot. Both transports use the same
calculation and on-chain completion path.

Each trade keeps its own buyer/seller penalty on-chain. A missing counterparty
reading does not remove that trade from the other facility's totals, and completing
one trade does not shrink the baseline or redistribute its share onto pending trades.

Totals reflect the currently indexed trade set. There is no slot-finalization
snapshot or correction/reconciliation mechanism: a subsequently indexed trade
or corrected reading can change calculated allocations, but verification cannot
revise amounts already recorded on-chain. Defining finalization and handling such
late changes remain follow-up work; slot-specific measurement selection alone
does not solve this limitation.

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

- CLI: `web3 <offchain_host> <offchain_port> <node_host> <node_port> <polling_interval> <market_duration> <penalty_rate> [--penalty-tolerance-percent <percent>]`
- Env:
  - `TRADE_SETTLEMENT_ADDRESS`
  - `EXECUTION_ENGINE_PRIVATE_KEY`
  - `EXECUTION_ENGINE_OFFSET_MIN`
  - `EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` (default `900`; set `0` to disable)
  - `EXECUTION_ENGINE_PENALTY_TOLERANCE_PERCENT` (default `5`; set `0` for strict comparison)

Penalty tolerance must be a finite percentage between 0 and 100. Invalid values
fail startup. The CLI option takes precedence over the environment variable.
