# GSY DEX Execution Engine

## Role in the System

`gsy-execution-engine` computes imbalance penalties after trade settlement and submits
penalty batches to `TradeSettlement`.

## Execution Cycle

Each cycle:

1. Select the current target timeslot or an older slot still within its rollover grace period.
2. Fetch trades plus measurement points/timeseries for that window from off-chain storage.
3. Compute penalties from traded vs measured energy delta.
4. Submit penalties to EVM.

## Penalty Computation

Current penalty calculator logic:

- `delta = measured_energy - traded_energy`
- `delta > 0`: penalize buyer
- `delta < 0`: penalize seller
- Penalty scaled with `NODE_FLOAT_SCALING_FACTOR` (`10000`)

## Duplicate Submission Protection

Before submitting, the engine checks on-chain `penaltyEnergyByTrade(tradeId)`:

- If value is non-zero, that trade penalty is skipped.
- Only new penalties are submitted in the transaction.

This prevents repeated submission across recurring execution cycles.

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
Execution and penalty submissions remain serial; the polling interval and
transport latency still determine how often each slot can be checked.

Empty results, failures, and successful submissions do not remove a retained
slot early: additional records can still arrive. The existing on-chain penalty
check prevents resubmission for trades already processed. The eligible window
is recalculated from time, including after restart or skipped polling intervals;
neither restarts nor retries extend the deadline. Records arriving beyond that
window require separate historical processing, which is not implemented here.

`EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` replaces the cycle-count setting
`EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT`; update existing environment overrides.
The old setting is ignored, not converted to seconds. Invalid grace-period
values fail startup rather than silently falling back to the default.

## Contract Interaction

- Role check: `hasRole(EXECUTION_ENGINE_ROLE, signer)`
- Submission call: `submitPenalties(tuple[])`
- Success criteria: transaction receipt status is `1`

## Key Config

- CLI: `web3 <offchain_host> <offchain_port> <node_host> <node_port> <polling_interval> <market_duration> <penalty_rate>`
- Env:
  - `TRADE_SETTLEMENT_ADDRESS`
  - `EXECUTION_ENGINE_PRIVATE_KEY`
  - `EXECUTION_ENGINE_OFFSET_MIN`
  - `EXECUTION_ENGINE_ROLLOVER_GRACE_SECONDS` (default `900`; set `0` to disable)
