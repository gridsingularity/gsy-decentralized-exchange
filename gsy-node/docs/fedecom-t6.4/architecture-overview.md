# FEDECOM T6.4 Architecture Overview

## 1. Purpose

This document gives a **high-level overview** of the FEDECOM T6.4 implementation
in the GSy node. It explains how the modules involved in T6.4 fit together and,
in particular, how the `remuneration` and `stripe-bridge` pallets interact.

It is intentionally written at the architecture/integration level. It does
**not** reproduce the low-level formulas, storage definitions, call indices,
event lists, or test enumerations that already live in the detailed module
documentation:

- `gsy-node/modules/remuneration/README.md`
- `gsy-node/modules/stripe-bridge/README.md`

This overview is a map, not a replacement for those READMEs. A reader who wants
exact formulas, storage layouts, or per-call semantics should open the relevant
module README after reading this document.

> Scope note: T6.4 is a **research/prototype** implementation for FEDECOM. It is
> not a production payment system, and this document does not claim production
> readiness.

## 2. Modules involved

| Module | Type | Main role in T6.4 |
|---|---|---|
| `remuneration` | On-chain FRAME pallet | On-chain ledger and flexibility settlement pallet; source of truth for balances, payments, and bridge escrow. |
| `stripe-bridge` | On-chain FRAME pallet with an off-chain-worker (OCW) courier | Owns the Stripe-backed bridge-transfer lifecycle on-chain and uses an OCW to talk to Stripe; source of truth for Stripe workflow status. |
| `trades-settlement` | On-chain FRAME pallet | Existing trade-settlement integration point; remains compatible with the remuneration changes. |
| `runtime` (`gsy-node-runtime`) | Node runtime | Wires the pallets into the node via their `Config` implementations and pallet indices. |
| `stripe_client` (inside `stripe-bridge`) | OCW-only HTTP client | Builds and performs Stripe REST calls; invoked **only** from the off-chain worker, never from deterministic runtime code. |

## 3. Architectural responsibility split

The design keeps responsibilities cleanly separated:

```text
Remuneration owns money and settlement state.
Stripe bridge owns Stripe workflow state.
The OCW talks to Stripe.
The runtime wires the pallets together.
```

Concretely:

- **`remuneration` is the source of truth** for balances, payments, and bridge
  escrow (the reserved/finalised/released funds backing a transfer).
- **`stripe-bridge` is the source of truth** for Stripe workflow status and the
  bridge-transfer lifecycle (creation, reservation, submission, finalisation,
  reversal, inbound confirmation).
- **Stripe HTTP calls never happen inside deterministic runtime execution.**
  All network I/O is confined to the off-chain worker.
- **OCW activity is off-chain** and reports its results back on-chain through
  extrinsics, so the canonical chain state stays deterministic and reproducible
  on every node.

## 4. Cross-pallet interaction

The dependency between the two pallets is one-way:

```text
stripe-bridge → remuneration
```

Key properties:

- The dependency is **one-directional**. `stripe-bridge::Config` requires
  `remuneration::Config`; `remuneration` does **not** depend on `stripe-bridge`.
- `stripe-bridge` calls **public helper functions** on
  `remuneration::Pallet<T>`.
- These are **synchronous Rust function calls inside the same runtime state
  transition**. They are **not** XCM, **not** asynchronous messages, and **not**
  separate extrinsics. If a remuneration helper returns an error, the enclosing
  dispatch is rolled back as a normal transactional failure.

The helper functions used (conceptually) are:

```text
query_custodian
bridge_reserve_funds
bridge_finalize_outbound
bridge_release_funds
bridge_credit_inbound
```

`query_custodian` lets the bridge authorize admin actions against the
remuneration custodian; the four `bridge_*` helpers move money through escrow
(reserve, finalise, release) and credit inbound transfers exactly once. See the
remuneration README for the exact signatures and semantics.

## 5. Canonical outbound flow

The canonical outbound flow moves value from the on-chain ledger toward Stripe:

```text
custodian request
→ Stripe bridge creates BridgeTransfer
→ remuneration reserves funds
→ OCW creates Stripe PaymentIntent
→ OCW submits unsigned-but-signed result
→ ValidateUnsigned verifies the payload
→ remuneration finalises or releases escrow
```

Simplified diagram:

```text
 Custodian        stripe-bridge (on-chain)        remuneration            OCW (off-chain)        Stripe
    |                     |                            |                       |                    |
    | request_transfer_to_stripe (SIGNED)              |                       |                    |
    |-------------------->| create BridgeTransfer       |                       |                    |
    |                     | bridge_reserve_funds ======>| (SYNC pallet call)    |                    |
    |                     |<===== Ok                    |                       |                    |
    |                     | status = FundsReserved      |                       |                    |
    |                     |                             |   reads pending work  |                    |
    |                     |------------------------------------------------------>| PaymentIntent (HTTP)
    |                     |                             |                       |------------------->|
    |                     |                             |                       |<----- result ------|
    |                     | submit_outbound_transfer_result                     |                    |
    |                     | (UNSIGNED extrinsic, payload SIGNED by OCW key)      |                    |
    |                     |<-----------------------------------------------------|                    |
    |                     | ValidateUnsigned verifies signature                 |                    |
    |                     | success -> bridge_finalize_outbound ===>|           |                    |
    |                     | failure -> bridge_release_funds      ===>|          |                    |
    |                     | status = Finalized | Reverted            |          |                    |
```

What matters at the architecture level:

- The **initial request is a signed on-chain extrinsic** authorized against the
  custodian.
- The **escrow reservation is synchronous and on-chain** (a direct call into
  remuneration).
- The **Stripe API call is off-chain**, performed by the OCW.
- The **result returns as an unsigned extrinsic carrying a signed payload**,
  verified by `ValidateUnsigned`.
- **Success finalises** the escrow; **failure releases** it.
- Because the external Stripe HTTP side effect **cannot be atomic with chain
  state**, the design uses a **reserve / finalise / release compensation
  pattern** rather than a single cross-system atomic transaction.

For the full lifecycle, status transitions, retry, and force-revert behaviour,
see the stripe-bridge README.

## 6. Canonical inbound flow

The inbound flow brings value from a Stripe-side action back into the ledger:

```text
custodian confirms Stripe-side transfer
→ Stripe bridge records inbound transfer
→ remuneration credits participant
→ external reference is marked consumed
```

Trust properties, stated explicitly:

- Inbound is **custodian-confirmed**: the custodian asserts that a Stripe-side
  transfer occurred.
- It does **not** currently use Stripe **webhooks**.
- It does **not** currently **verify the Stripe object through the OCW**.
- **Duplicate external references are rejected by remuneration**, so a given
  reference can credit a participant only once.

This is acceptable for the research-prototype scope, but it is **not
production-grade inbound payment verification**.

## 7. Legacy versus canonical Stripe paths

Two workflow families coexist in the stripe-bridge pallet:

- The **canonical bridge-transfer flow** is the remuneration-integrated FEDECOM
  flow described in Sections 5 and 6. It reserves, finalises, releases, and
  credits funds through remuneration escrow.
- The **legacy payment/refund/balance queues** remain for compatibility and
  demo purposes. They record Stripe payment/refund/balance state on-chain but
  **do not reserve funds in remuneration**.

The final decision on whether to **keep, deprecate, remove, or migrate** the
legacy queues should be discussed with GSy during integration. This document
does not recommend removing them.

## 8. On-chain/off-chain boundary

- **Deterministic runtime code manages state.** It runs identically on every
  node and never performs network I/O.
- **OCW code performs HTTP calls** to Stripe via the `stripe_client`.
- The **Stripe API key is node-local** (off-chain storage) and is **not stored
  on-chain**.
- The OCW uses the **`strp` key** to sign result payloads.
- Result extrinsics are **unsigned at the FRAME level** but **signed at the
  payload level**.
- **`ValidateUnsigned`** verifies those payload signatures before the call is
  admitted to the transaction pool.
- Recent tests cover the real `ValidateUnsigned` path (valid signatures,
  bad-proof rejection, payload mismatch, and validate-then-dispatch).

## 9. Runtime integration

- The runtime **mounts both pallets** (`Remuneration` and `StripeBridge`) and
  also `TradesSettlement`.
- `remuneration` and `stripe-bridge` are wired through their `Config`
  implementations in the runtime.
- `stripe-bridge` **requires the remuneration `Config`** to be present in the
  same runtime.
- OCW transaction submission requires the runtime to provide the **signed
  transaction support** (`CreateSignedTransaction` / `SendTransactionTypes`) so
  the OCW can construct and submit unsigned result extrinsics; the unsigned
  priority is configured at `1 << 20`.
- **Pallet indices are branch-local.** On `fedecom-T6.4` the runtime declares
  `Remuneration` at index 11 and `StripeBridge` at index 13 (with a gap). These
  indices and the final wiring **must be confirmed with GSy before main-branch
  integration** and should not be treated as fixed main-branch values.

## 10. Role of `trades-settlement`

`trades-settlement` is an **existing settlement integration point**, not a new
T6.4 module. It already depends on `remuneration` (its `Config` requires
`remuneration::Config`) and settles matched trades through the remuneration
ledger via the `RemunerationHandler` interface.

For T6.4 it is relevant mainly because it **remains compatible** with the
remuneration changes. The T6.4 work did not materially change its logic.
Regression tests for `trades-settlement` are run to confirm it remains
unaffected by the remuneration and bridge changes.

## 11. Validation status

Latest verified status at a high level:

```text
remuneration: 109 tests passing
stripe-bridge: 77 tests passing
trades-settlement: 3 tests passing
runtime check: passing
```

Note that workspace-wide `cargo fmt --all -- --check` may still fail due to
**pre-existing, unrelated formatting debt** in the repository. This is observed
state and is not hidden here; it is not introduced by the T6.4 work and is not
fixed as part of this documentation task.

## 12. Research scope and deferred production features

### Research-scope limitations

- Inbound confirmation is **custodian-trusted** (no independent on-chain
  verification of the Stripe event).
- Stripe communication targets the **sandbox / research** environment.
- **Force-revert** of a stuck outbound transfer requires **operational care**,
  because the on-chain release is not automatically reconciled against
  Stripe-side state.
- **Legacy queues are retained** alongside the canonical flow.

### Future production features

- Stripe **Connect**.
- Stripe **webhooks** (ingestion and verification).
- Full **reconciliation** and audit tooling.
- A real **bank payout** flow.
- **KYC/KYB**.
- **Benchmark-generated production weights**.
- A **provider-neutral abstraction** (e.g. a payment-provider/settlement trait)
  instead of direct coupling.

### Integration topics for GSy

- Final **branch integration strategy** (single PR vs. separate PRs).
- Final **pallet indices**.
- **Direct remuneration dependency versus a trait abstraction** between pallets.
- **Fate of the legacy queues** (keep / deprecate / remove / migrate).
- Operational **API-key and `strp` key setup** for participating nodes.
- **Event-correlation expectations** (notably the `bridge-transfer-{id}` escrow
  naming convention).
- **CI / rustfmt standards** for the repository.

## 13. Links to detailed documentation

For module-level detail, see:

- `gsy-node/modules/remuneration/README.md`
- `gsy-node/modules/stripe-bridge/README.md`

The trade-settlement integration point lives in:

- `gsy-node/modules/trades-settlement/`

This overview deliberately does not duplicate those documents; they remain the
authoritative source for formulas, storage, events, call indices, and tests.
