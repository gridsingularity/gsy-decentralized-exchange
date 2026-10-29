# GSY DEX Execution Engine

## Role in the System

`gsy-execution-engine` computes imbalance penalties after trade settlement and
records delivery verification in `TradeSettlement`.

## Execution Cycle

Each cycle:

1. Determine target timeslot using configured offset.
2. Fetch trades plus measurement points/timeseries for that window from off-chain storage.
3. Compute verdicts for `Settled` trades with a usable measurement.
4. Submit `completeDeliveryVerification` and wait for a successful receipt.
   Penalties (if any) and completion are recorded atomically. Fully delivered
   trades also complete, without a separate penalty transaction.

## Penalty Computation

Current penalty calculator logic:

- `delta = measured_energy - traded_energy`
- `delta > 0`: penalize buyer
- `delta < 0`: penalize seller
- Penalty scaled with `NODE_FLOAT_SCALING_FACTOR` (`10000`)

The existing policy is intentionally unchanged: measurements are mapped by
facility owner; the buyer's measurement is preferred, with the seller's as a
fallback. This does not independently verify both sides. Multiple measurements
for an owner overwrite each other, and allocation across multiple trades is not
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

An EWDS request can remain in flight while the configured target delivery slot
advances. The engine retains the outgoing target slot for a bounded number of
additional cycles so trades indexed around the boundary are not skipped. It
stops retrying when all currently fetched settled trades have measurements and
their verification is confirmed on-chain. Verifying only a subset does not
release the retained slot. The existing retry limit still applies; durable
recovery of records arriving after that limit is outside this change.

## Contract Interaction

- Required role: `EXECUTION_ENGINE_ROLE`
- Submission call: `completeDeliveryVerification(tuple[])`
- Success criteria: transaction receipt status is `1`

The engine does not write lifecycle statuses through HTTP/EWDS. The off-chain
storage listener indexes `DeliveryVerified` as `Executed` for the trade and its
two consumed orders. Residual order IDs remain independent.

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
  - `EXECUTION_ENGINE_ROLLOVER_RETRY_LIMIT` (default `2`; set `0` to disable)
