# GSY DEX Execution Engine

## Role in the System

`gsy-execution-engine` computes imbalance penalties after trade settlement and submits
penalty batches to `TradeSettlement`.

## Execution Cycle

Each cycle:

1. Determine target timeslot using configured offset.
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

An EWDS request can remain in flight while the configured target delivery slot
advances. The engine retains the outgoing target slot for a bounded number of
additional cycles so trades indexed around the boundary are not skipped. It
stops retrying as soon as penalties for that slot are submitted or found
on-chain.

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
  - `EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT` (default `2`; set `0` to disable)
