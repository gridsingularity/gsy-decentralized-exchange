# Stripe Bridge Pallet

> Scope note: this document describes a **research/prototype** bridge built for
> FEDECOM. It is not a complete production payment system. See
> [Section 15](#15-known-limitations-and-deferred-production-features) for the
> full list of research-scope limitations and deferred production features.

> For a high-level, cross-module view of how this pallet fits into FEDECOM T6.4 (including its interaction with `remuneration`), see [`gsy-node/docs/fedecom-t6.4/architecture-overview.md`](../../docs/fedecom-t6.4/architecture-overview.md).

## 1. Purpose and scope

This pallet demonstrates a Stripe-backed bridge workflow for FEDECOM research
purposes. The canonical flow connects Stripe-side actions to the remuneration
pallet through on-chain escrow records. It should not be interpreted as a
complete production payment system.

Concretely:

- the pallet demonstrates communication between the blockchain runtime and the
  **Stripe sandbox**;
- the canonical FEDECOM flow integrates with the `remuneration` pallet, which
  remains the source of truth for the project's virtual ledger;
- this is a **research/prototype** bridge, not a production payment product.

It does **not** implement:

- Stripe Connect;
- real participant payouts;
- Stripe webhooks;
- bank transfers;
- KYC/KYB;
- full production reconciliation.

The most important structural point: the Stripe bridge is

```text
an on-chain FRAME pallet with an off-chain-worker (OCW) courier
```

It is **not** simply an "off-chain pallet". The on-chain pallet owns storage,
extrinsics, the transfer lifecycle, the unsigned-result calls, events,
validation, and the integration with remuneration. The off-chain worker is a
courier that reads pending work, talks to Stripe over HTTP, signs result
payloads, and submits them back as unsigned extrinsics.

## 2. Component model

The system has four layers:

```text
Runtime / on-chain pallet   (deterministic state, storage, extrinsics, ValidateUnsigned)
Off-chain worker (OCW)       (non-deterministic courier: reads work, calls Stripe, submits results)
Stripe HTTP client           (used only by the OCW)
Remuneration pallet          (on-chain source of truth for balances/escrow)
```

| Component | Runs where | Main responsibility |
| --------- | ---------- | ------------------- |
| On-chain pallet (`pallet`) | Runtime, on every node, deterministically | Owns storage, custodian extrinsics, canonical transfer lifecycle, unsigned-result calls, events, `ValidateUnsigned`, and synchronous calls into remuneration. |
| Off-chain worker (`offchain_worker` hook) | Off-chain, only on nodes that run OCWs and hold a key | Reads pending work from on-chain storage, reads the node-local Stripe API key, calls Stripe over HTTP, signs result payloads, submits unsigned result extrinsics. |
| Stripe HTTP client (`src/stripe_client.rs`) | Off-chain only (invoked by the OCW) | Builds and performs Stripe REST calls and extracts fields from JSON responses. |
| Remuneration pallet (`remuneration`) | Runtime, on-chain | Source of truth for balances and bridge escrow; exposes synchronous helper functions consumed by this pallet. |

Explicit boundaries:

- **on-chain code is deterministic** and runs identically on every node;
- **HTTP is only performed off-chain**, inside the OCW, via the Stripe HTTP
  client; on-chain code never performs network I/O;
- the chain receives OCW results through **signed payloads submitted as
  unsigned extrinsics** (FRAME-level unsigned, with an application-level
  signature over the payload that `ValidateUnsigned` verifies).

## 3. Relationship with remuneration

- `stripe-bridge` **depends on** `remuneration` (its `Config` requires
  `remuneration::Config`).
- The dependency is **one-way**: remuneration does not depend on stripe-bridge.
- Stripe bridge calls **public helper functions** on `remuneration::Pallet<T>`.
- These are **synchronous Rust calls inside the same runtime state
  transition**. They are **not** async messaging, **not** XCM, and **not** a
  second extrinsic. If a remuneration helper returns an error, the whole
  dispatch (and its storage changes) is rolled back as a normal
  `DispatchResult`/`#[transactional]` failure.
- `remuneration` remains the **source of truth for money and escrow**.
- `stripe-bridge` remains the **source of truth for Stripe workflow status**
  (the `BridgeTransfer` lifecycle and legacy queues).

Helpers used (conceptually):

| Helper | Purpose |
| ------ | ------- |
| `query_custodian` | Returns the remuneration custodian; used to authorize admin extrinsics. |
| `bridge_reserve_funds` | Reserves an owner's funds into bridge escrow before an outbound Stripe attempt. |
| `bridge_finalize_outbound` | Finalises the reserved debit after a successful outbound Stripe result. |
| `bridge_release_funds` | Releases reserved funds back to the owner after failure or admin revert. |
| `bridge_credit_inbound` | Credits an owner exactly once for a trusted inbound Stripe confirmation, keyed by external reference. |

Remuneration does **not** depend on Stripe bridge in any direction.

## 4. Canonical outbound flow

The main FEDECOM outbound (`remuneration -> Stripe`) flow:

```text
request_transfer_to_stripe
→ create BridgeTransfer
→ remuneration bridge_reserve_funds
→ status FundsReserved
→ OCW sees pending transfer
→ OCW creates Stripe PaymentIntent
→ OCW submits submit_outbound_transfer_result
→ ValidateUnsigned checks signature
→ success: remuneration bridge_finalize_outbound
→ failure: remuneration bridge_release_funds
```

Sequence diagram:

```text
 Custodian        stripe-bridge (on-chain)        remuneration         OCW (off-chain)        Stripe
    |                     |                            |                     |                   |
    |  request_transfer_to_stripe (SIGNED extrinsic)   |                     |                   |
    |-------------------->|                            |                     |                   |
    |                     |  create BridgeTransfer      |                     |                   |
    |                     |  bridge_reserve_funds  ====>|  (SYNC pallet call) |                   |
    |                     |<==== Ok                     |                     |                   |
    |                     |  status = FundsReserved      |                     |                   |
    |                     |                            |                     |                   |
    |                     |  (state visible on-chain)   |   reads pending work|                   |
    |                     |---------------------------------------------------> |                  |
    |                     |                            |                     | create PaymentIntent (HTTP)
    |                     |                            |                     |------------------>|
    |                     |                            |                     |<------ result ----|
    |                     |  submit_outbound_transfer_result                  |                   |
    |                     |  (UNSIGNED extrinsic, payload SIGNED by OCW key)   |                   |
    |                     |<--------------------------------------------------|                   |
    |                     |  ValidateUnsigned: verify payload signature        |                   |
    |                     |  success -> bridge_finalize_outbound ====>|        |                   |
    |                     |  failure -> bridge_release_funds      ====>|       |                   |
    |                     |  status = Finalized | Reverted             |       |                   |
```

The four distinct interaction types in this flow are intentionally different:

1. **Signed custodian extrinsic** — `request_transfer_to_stripe`, authorized
   against the remuneration custodian.
2. **Synchronous pallet-to-pallet call** — `bridge_reserve_funds`,
   `bridge_finalize_outbound`, `bridge_release_funds` execute inside the same
   on-chain state transition.
3. **OCW HTTP request** — the off-chain worker calls Stripe; this is
   non-deterministic and cannot run on-chain.
4. **Unsigned-but-signed result extrinsic** — `submit_outbound_transfer_result`
   is FRAME-unsigned, but its payload is signed by the OCW key and verified by
   `ValidateUnsigned`.
5. **On-chain finalisation/release** — the result handler drives the escrow
   finalise (success) or release (failure).

Because Stripe HTTP side effects **cannot be atomic with chain state**, the
design uses a **reserve / finalise / release compensation** model rather than
attempting a single atomic transaction across the chain and Stripe.

## 5. Canonical inbound flow

The inbound (`Stripe -> remuneration`) flow:

```text
confirm_transfer_from_stripe
→ create inbound BridgeTransfer
→ remuneration bridge_credit_inbound
→ mark external reference consumed
→ finalised on-chain (CreditedOnChain → Finalized)
```

Trust properties (stated explicitly):

- this flow is **custodian-confirmed**;
- it does **not** currently use a Stripe webhook;
- it does **not** currently have OCW verification of the Stripe object;
- the chain **trusts the custodian-provided** amount, external reference, and
  Stripe object ID;
- **duplicate external references are rejected by remuneration**
  (`bridge_credit_inbound` returns `BridgeDuplicateExternalCredit`, surfaced
  here as `DuplicateInboundExternalReference`), so a reference can only credit
  once.

This is **not** a production-grade inbound verification flow. It is a trusted
custodian confirmation suitable for the research prototype.

## 6. Retry and force revert

`retry_transfer_to_stripe`:

- used when a canonical outbound transfer has reached a failed/reverted
  terminal state and the custodian wants to try again;
- it does **not** recycle the old transfer in place: it creates a **fresh
  linked transfer** with `retry_of = Some(original_bridge_id)` and a fresh
  escrow reservation;
- the original transfer remains as historical state;
- a given original transfer can only be retried once (a retry descendant blocks
  further retries), and a successfully finalised transfer cannot be retried.

`force_revert_outbound_transfer`:

- operational recovery action for a **stuck** outbound transfer;
- it **releases reserved funds** in remuneration and marks the transfer
  `Reverted`;
- **eligible states**: `FundsReserved` and `SubmittedToStripe` (outbound
  direction only);
- **protected states**: a successfully finalised transfer is rejected
  (`OutboundTransferNotForceRevertable`), preventing double settlement;
- force revert after external Stripe activity (e.g. when the transfer already
  reached `SubmittedToStripe`) is a **research/prototype recovery mechanism**
  and requires **operational care**, because the on-chain release is not
  automatically reconciled against any Stripe-side state.

## 7. Legacy Stripe queues versus canonical bridge transfers

Two workflow families coexist.

**Legacy queue-based workflows** (not remuneration-integrated):

```text
queue_stripe_payment
queue_stripe_refund
request_balance_check
submit_payment_result
submit_refund_result
submit_balance_result
PendingPayments / ProcessedPayments
PendingRefunds / ProcessedRefunds
LastBalance
```

**Canonical bridge workflows** (remuneration-integrated, FEDECOM):

```text
request_transfer_to_stripe
confirm_transfer_from_stripe
retry_transfer_to_stripe
force_revert_outbound_transfer
submit_outbound_transfer_result
BridgeTransfers
remuneration escrow (reserve / finalise / release / credit)
```

Clear statements:

- the **canonical flow is the remuneration-integrated FEDECOM flow**;
- **legacy queues do not reserve funds in remuneration** — they only record
  Stripe payment/refund/balance state on-chain;
- legacy queues are **retained for compatibility / demo functionality**;
- a future GSy discussion should decide whether to **keep, deprecate, remove, or
  migrate** them;
- they are **not removed in this task**.

## 8. Off-chain worker and Stripe API key

- The OCW reads the Stripe API key from **node-local off-chain persistent
  storage**; it is read as raw bytes (not SCALE-encoded).
- The API key is **not stored on-chain**.
- The off-chain storage key used by the pallet is:

  ```text
  stripe-bridge::api-key
  ```

  (constant `STRIPE_API_KEY_STORAGE`).
- Each participating node must be configured with the key **if it is expected
  to perform Stripe calls**.
- A **missing API key causes the OCW to skip** Stripe processing for that block
  (it logs a warning and returns).
- OCW processing therefore depends on **node configuration and off-chain worker
  execution**: a node that does not run the OCW, or has no key, simply does not
  act as a courier.

This document does not define deployment scripts; none are invented here.

## 9. OCW signing key and unsigned result extrinsics

- Result calls are **unsigned extrinsics at the FRAME level** (the dispatch path
  only calls `ensure_none(origin)`).
- The **payload itself is signed** by the OCW application key, and the signature
  is carried alongside the payload.
- The OCW signing key type is:

  ```text
  KEY_TYPE = strp
  ```

- `ValidateUnsigned` verifies the signed payload
  (`SignedPayload::verify::<T::AuthorityId>`); a bad or mismatched signature is
  rejected with `InvalidTransaction::BadProof`.
- **Dispatch itself only checks `ensure_none`** and does not re-verify the
  signature; therefore **transaction-pool validation is essential** — it is the
  layer that authenticates the payload before the call can be included.
- Tests now cover the real `ValidateUnsigned` path (see
  [Section 14](#14-tests-and-validation)).

Accepted result transactions share this metadata: tag prefix `stripe-bridge`,
priority = `T::UnsignedPriority` (1<<20 in the current configs), longevity = 3,
`propagate = true`.

The four unsigned result calls:

| Call | Payload | `provides` tag | Duplicate behaviour | Relevant on-chain guard |
| ---- | ------- | -------------- | ------------------- | ----------------------- |
| `submit_payment_result` | `PaymentResultPayload` (`payment_index`, `stripe_payment_id`, `status`, `gross_amount`, `stripe_fee`, `net_amount`, `public`) | per `payment_index` | two pool txs for the same index collide; different indexes are independent | removes `PendingPayments[idx]`, inserts `ProcessedPayments[idx]` (last-write-wins; no escrow effect) |
| `submit_refund_result` | `RefundResultPayload` (`refund_index`, `refund_id`, `status`, `amount`, `public`) | per `refund_index` | same as above, keyed by refund index | removes `PendingRefunds[idx]`, inserts `ProcessedRefunds[idx]` |
| `submit_balance_result` | `BalanceResultPayload` (`available_*`, `pending_*`, `public`) | **singleton** literal `submit_balance_result` | any two pending balance results collide regardless of content | clears `BalanceCheckRequested`, overwrites `LastBalance` |
| `submit_outbound_transfer_result` | `OutboundTransferResultPayload` (`bridge_id`, `success`, `stripe_object_id`, `stripe_status`, `error_message`, `public`) | per `bridge_id` | two pool txs for the same bridge id collide; different ids independent | requires `ToStripe` + `FundsReserved`; finalises (success) or releases (failure) escrow; a second submission is rejected on-chain with `InvalidBridgeTransferStatusTransition` |

Important distinctions:

- `provides` tags are **transaction-pool duplicate controls**, not settlement
  guarantees;
- **on-chain state guards are still required** and are the authoritative
  protection against double finalisation/release;
- the **balance result uses a singleton `provides` tag** (intentional: balance
  is modeled as a singleton request/result flow);
- the **canonical outbound result uses the bridge ID** in its `provides` tag.

## 10. Idempotency and duplicate protection

- **Canonical outbound** uses a deterministic Stripe **idempotency key** derived
  from the bridge ID (`stripe-bridge-outbound-{bridge_id}`), so a repeated OCW
  attempt for the same transfer does not create a second Stripe PaymentIntent.
- **Local in-flight markers** (off-chain persistent storage, prefix
  `stripe-bridge::outbound-transfer-in-flight`, ~60s TTL) reduce duplicate OCW
  attempts within the cooldown window.
- **`provides` tags** reduce duplicate submissions inside the transaction pool.
- **On-chain status guards** prevent double finalisation/release (the
  authoritative protection).
- **Legacy flows may not have the same idempotency guarantees** as the canonical
  bridge flow (e.g. legacy payment creation does not use a bridge-ID idempotency
  key).

These are layered, best-effort controls for a prototype; they are not a claim of
production-grade exactly-once payment guarantees.

## 11. Storage overview

On-chain storage:

| Storage | Canonical / Legacy | On-chain | Purpose |
| ------- | ------------------ | -------- | ------- |
| `StripeEnabled` | shared | yes | Global on/off switch for the bridge's operational behaviour. |
| `PendingPayments` | legacy | yes | Queued legacy Stripe payment requests awaiting OCW processing. |
| `ProcessedPayments` | legacy | yes | Recorded results of legacy Stripe payments. |
| `NextPaymentIndex` | legacy | yes | Monotonic id source for legacy payments. |
| `PendingRefunds` | legacy | yes | Queued legacy Stripe refund requests. |
| `ProcessedRefunds` | legacy | yes | Recorded results of legacy refunds. |
| `NextRefundIndex` | legacy | yes | Monotonic id source for legacy refunds. |
| `BridgeTransfers` | canonical | yes | Canonical `BridgeTransfer` records keyed by `bridge_id`. |
| `NextBridgeTransferId` | canonical | yes | Monotonic id source for canonical bridge transfers. |
| `LastBalance` | legacy | yes | Latest Stripe balance snapshot. |
| `BalanceCheckRequested` | legacy | yes | Flag indicating a balance check should be executed by the OCW. |

Off-chain local storage (node-local, **not** on-chain):

| Off-chain item | Key / constant | Purpose |
| -------------- | -------------- | ------- |
| Stripe API key | `stripe-bridge::api-key` (`STRIPE_API_KEY_STORAGE`) | Secret used by the OCW to authenticate Stripe HTTP calls. |
| Outbound in-flight markers | prefix `stripe-bridge::outbound-transfer-in-flight` (`OUTBOUND_TRANSFER_IN_FLIGHT_PREFIX`) + bridge id | Short-TTL guard to avoid immediate duplicate OCW processing of the same outbound transfer. |
| Balance-check flag constant | `stripe-bridge::balance-check` (`BALANCE_CHECK_FLAG`) | A defined off-chain key constant. Note: the **authoritative** balance-check flag in the current code is the **on-chain** `BalanceCheckRequested` storage value; the OCW reads that on-chain flag. The off-chain constant is defined but not used as the active flag. |

The canonical `BridgeTransfer` record stores: `owner`, `amount`, `currency`,
`direction`, `status`, `retry_of`, `stripe_object_id`, `external_reference`,
`escrow_reference`, `last_error`. The two reference fields are intentionally
separate: `external_reference` is the Stripe/business-side identity used by the
inbound flow, while `escrow_reference` is the internal remuneration escrow
reference for outbound transfers (typically `bridge-transfer-{id}`).

## 12. Events and observability

| Category | Events |
| -------- | ------ |
| Operational toggle | `StripeToggled` |
| Canonical transfer lifecycle | `BridgeTransferCreated`, `BridgeTransferStatusUpdated` |
| Canonical outbound | `OutboundTransferToStripeRequested`, `OutboundTransferToStripeSucceeded`, `OutboundTransferToStripeFailed` |
| Canonical inbound | `InboundTransferFromStripeConfirmed`, `InboundTransferFromStripeCredited` |
| Retry / force revert | `OutboundTransferRetried`, `OutboundTransferForceReverted` |
| Legacy payment/refund/balance | `StripePaymentQueued`, `StripePaymentProcessed`, `StripePaymentFailed`, `StripeRefundQueued`, `StripeRefundProcessed`, `BalanceCheckRequested`, `StripeBalanceUpdated` |

Correlating a canonical transfer end-to-end:

- the numeric **`bridge_id`** is the primary key in `BridgeTransfers` and
  appears in the canonical events;
- the **escrow reference** in remuneration follows the naming convention
  `bridge-transfer-{bridge_id}` — this is a **naming convention**, not a stored
  cross-index, so correlation between a bridge transfer and its remuneration
  escrow currently relies on reconstructing that string;
- the **Stripe object ID** (`stripe_object_id`) links the transfer to the Stripe
  PaymentIntent/refund object;
- the **external reference** (`external_reference`) links an inbound transfer to
  the trusted Stripe/business-side reference and is the deduplication key in
  remuneration.

Because the escrow correlation relies on a naming convention, any change to that
convention must be coordinated with remuneration.

## 13. Runtime integration

- **Pallet index**: in this branch the runtime declares
  `#[runtime::pallet_index(13)] pub type StripeBridge = stripe_bridge;`
  (Remuneration is index 11). The index has a gap and is **branch-local**; the
  **final pallet index layout must be confirmed during integration** with GSy
  and should not be treated as the main-branch index.
- **`stripe_bridge::Config`** requires:
  `frame_system::offchain::CreateSignedTransaction<Call<Self>>` +
  `frame_system::offchain::SendTransactionTypes<Call<Self>>` +
  `frame_system::Config` + `remuneration::Config`.
- **`AuthorityId`**: `AppCrypto<Self::Public, Self::Signature>` — the OCW signing
  identity (key type `strp`).
- **Unsigned priority**: `UnsignedPriority` is configured at `1 << 20` in both
  the runtime and the test mock.
- The runtime provides `CreateSignedTransaction` / `SendTransactionTypes`
  support so the OCW can construct and submit the unsigned result extrinsics.
- The pallet depends on the **remuneration `Config`** being present in the same
  runtime.

Final pallet index and runtime wiring must be confirmed during main-branch
integration; the values above describe the current `fedecom-T6.4` branch only.

## 14. Tests and validation

Current test coverage (in `src/tests.rs`, against the mock runtime in
`src/mock.rs`):

- **Legacy queue tests** — queueing payments/refunds, index increments,
  custodian access control, disabled/long-currency rejection, and legacy result
  storage.
- **Canonical bridge transfer tests** — creation and stored-field correctness,
  strict direction-aware lifecycle transitions, outbound success/failure,
  retry lineage, and force-revert behaviour.
- **OCW mocked HTTP tests** — the off-chain worker is exercised against mocked
  Stripe responses for outbound transfers, payments, refunds, and balance
  checks, plus the in-flight marker / API-key / disabled-bridge skip paths.
- **Remuneration escrow integration tests** — reserve / finalise / release and
  inbound exactly-once credit effects are asserted through the remuneration
  pallet from within the mock runtime.
- **`ValidateUnsigned` tests** — the transaction-pool validation path is now
  exercised directly.
- **Runtime check** — `cargo check -p gsy-node-runtime` confirms the pallet
  still integrates into the runtime.

The `ValidateUnsigned` test pass specifically covers:

- **valid signatures** accepted with the expected priority/longevity/propagate
  and a non-empty `provides` tag;
- **invalid signatures** rejected with `InvalidTransaction::BadProof`;
- **payload mismatch** (signing one logical id and validating another) rejected
  with `BadProof`, proving the payload contents are authenticated;
- **`provides` tags** — unique per logical id for payment/refund/outbound, and a
  singleton tag for balance;
- **non-result call rejection** (`InvalidTransaction::Call`);
- **validate-then-dispatch** of the canonical outbound result, proving the same
  call is valid at the pool layer and finalises escrow when dispatched.

## 15. Known limitations and deferred production features

**Research-scope limitations** (true of the current prototype):

- the inbound flow is **custodian-trusted** (no independent verification of the
  Stripe event on-chain);
- there is **no Stripe webhook verification**;
- there is **no Stripe Connect** integration;
- there is **no real bank payout flow**;
- there is **no full production reconciliation**;
- **legacy queues are retained** and coexist with the canonical flow; they
  should be reviewed;
- **placeholder weights** (`src/weights.rs`) are development weights and may
  require benchmarking for production;
- **force revert after external activity** requires operational care because the
  on-chain release is not reconciled against Stripe-side state;
- **multi-validator behaviour** relies on idempotency keys, `provides` tags,
  on-chain state guards, and operational setup rather than a single
  authoritative coordinator;
- the `AwaitingConfirmation` status still exists in the enum but is not used by
  the current trusted inbound flow;
- this is **not a production payment product**.

**Future production features** (deferred, not implemented here):

- Stripe webhook ingestion and verification;
- OCW (or webhook) verification of the Stripe object for inbound credits;
- Stripe Connect and real participant payouts;
- bank-transfer settlement;
- KYC/KYB;
- full reconciliation/ledger-audit tooling;
- benchmarked weights.

**Integration topics for GSy** (see also Section 16):

- whether to keep direct remuneration coupling or introduce a trait
  abstraction;
- the fate of the legacy queues;
- final pallet index;
- operational key and API-key provisioning;
- event-correlation expectations;
- whether inbound needs OCW/webhook verification before merge.

## 16. Integration questions for GSy

Technical questions to resolve for main-branch integration:

1. Keep the **direct coupling to remuneration**, or introduce a **trait
   abstraction** (payment-provider / settlement interface) between the pallets?
2. **Keep, deprecate, remove, or migrate** the legacy Stripe queues
   (`PendingPayments` / `PendingRefunds` / balance-check)?
3. Confirm the **final pallet index** (currently branch-local index 13).
4. Confirm **operational key setup** for the OCW signing key (`strp`).
5. Confirm **API-key provisioning** for participating nodes (off-chain local
   storage under `stripe-bridge::api-key`).
6. Confirm **event-correlation expectations** (notably the
   `bridge-transfer-{id}` escrow-reference naming convention vs. a stored
   cross-index).
7. Confirm **weights / benchmarking expectations** for the extrinsics.
8. Confirm whether **inbound needs OCW/webhook verification** before merge, or
   whether custodian-trusted confirmation is acceptable for the FEDECOM
   milestone.
9. Confirm whether this lands as **one PR or separate PRs** (e.g. canonical flow
   vs. legacy-queue cleanup).
