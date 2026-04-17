# Stripe Bridge Pallet

## Purpose

`stripe-bridge` is an offchain-worker-enabled Substrate pallet that coordinates interactions between:

- the on-chain `remuneration` pallet, which is the source of truth for the project's virtual ledger
- Stripe, which is treated as an external payment and settlement service

The pallet sits in the middle. It stores on-chain requests and settlement records, lets the offchain worker call Stripe over HTTP, and applies the resulting state changes back on chain.

The implementation currently contains two layers that coexist:

- a legacy queue-based Stripe integration for payments, refunds, and balance checks
- a newer canonical `BridgeTransfer` workflow for outbound remuneration -> Stripe settlement and inbound Stripe -> remuneration crediting

For remuneration integration, the pallet uses helper methods already exposed by `remuneration` for:

- outbound reserve / finalize / release through bridge escrow
- inbound exactly-once crediting based on an external reference

This pallet is not a native token bridge. It is a bridge/orchestration layer between an internal on-chain remuneration ledger and an external payment processor.

## Architecture Overview

### On-chain logic

On chain, the pallet provides:

- custodian-controlled extrinsics
- storage for legacy pending/processed Stripe payment and refund queues
- storage for balance snapshots
- a canonical `BridgeTransfer` record and lifecycle
- admin recovery actions for canonical outbound transfers

The canonical `BridgeTransfer` model is the pallet's future-facing settlement record. It is kept alongside the older queue-based storage so the pallet can evolve incrementally without removing existing behavior.

### Offchain worker

The offchain worker:

- reads the Stripe API key from local offchain persistent storage
- scans on-chain state for work
- performs HTTP requests to Stripe via `src/stripe_client.rs`
- submits unsigned extrinsics carrying signed payloads back on chain

In the current code, the offchain worker processes:

- canonical outbound transfers
- legacy queued payments
- legacy queued refunds
- balance checks

For canonical outbound transfers, the offchain worker also uses:

- a deterministic Stripe `Idempotency-Key`
- a node-local persistent in-flight guard to reduce immediate reprocessing

### Remuneration integration

The `remuneration` pallet remains responsible for the economic ledger semantics. `stripe-bridge` orchestrates the external interaction and delegates balance-side effects to remuneration helpers:

- outbound reserve of funds before Stripe submission
- outbound finalization of the reserved debit after Stripe success
- outbound release of reserved funds after Stripe failure or admin revert
- inbound idempotent crediting after trusted Stripe-side confirmation

### Admin and recovery layer

For canonical outbound transfers, the pallet includes a small operational recovery layer:

- retry of failed/reverted transfers through a fresh transfer record
- force-revert of stuck in-flight outbound transfers

This is additive operational tooling, not a redesign of settlement.

## Base Behavior

The pallet coordinates state transitions between:

- the remuneration virtual ledger on chain
- Stripe-side payment, refund, and balance APIs

It does not move native chain assets.

### Canonical outbound flow

The canonical outbound remuneration -> Stripe path works as follows:

1. A trusted caller requests an outbound transfer with `request_transfer_to_stripe`.
2. The pallet creates a canonical `BridgeTransfer`.
3. Funds are reserved in `remuneration` using the bridge escrow helper.
4. The transfer enters `FundsReserved`.
5. The offchain worker picks the transfer, derives a deterministic Stripe idempotency key, and calls Stripe.
6. The offchain worker submits the result back on chain through `submit_outbound_transfer_result`.
7. On Stripe success, the pallet finalizes the reserved debit in `remuneration` and finalizes the transfer.
8. On Stripe failure, the pallet releases the reserved funds in `remuneration` and marks the transfer as reverted.

### Canonical inbound flow

The canonical inbound Stripe -> remuneration path is a trusted on-chain confirmation flow:

1. A trusted caller reports a confirmed Stripe-side payment through `confirm_transfer_from_stripe`.
2. The pallet creates a canonical inbound `BridgeTransfer`.
3. The pallet calls the remuneration inbound credit helper using the external reference.
4. If the reference has not already been consumed, remuneration credits the owner's balance exactly once.
5. The transfer is marked `CreditedOnChain` and then `Finalized`.
6. If the reference was already used, the call fails and the duplicate transfer is not persisted.

### Legacy queue-based flow

The legacy queue-based flow still exists and is active for:

- queued Stripe payments
- queued Stripe refunds
- Stripe balance checks

That legacy storage and OCW path have not been removed. The canonical flow exists alongside it.

## Main Extrinsics and Main Functions

### User and admin-facing extrinsics

- `set_stripe_enabled`
  Enables or disables the Stripe bridge globally. Only the remuneration custodian may call it.

- `queue_stripe_payment`
  Queues a legacy Stripe payment request in `PendingPayments`.

- `queue_stripe_refund`
  Queues a legacy Stripe refund request for a previously processed payment.

- `request_balance_check`
  Marks that a Stripe balance snapshot should be fetched by the offchain worker.

- `request_transfer_to_stripe`
  Starts a canonical outbound bridge transfer. It creates a canonical transfer record and reserves remuneration funds through bridge escrow.

- `confirm_transfer_from_stripe`
  Records a trusted inbound Stripe confirmation and performs exactly-once remuneration crediting.

- `retry_transfer_to_stripe`
  Retries a failed/reverted canonical outbound transfer by creating a new transfer with fresh lineage and a fresh escrow reservation.

- `force_revert_outbound_transfer`
  Operational recovery action for stuck outbound transfers. It releases escrow and marks the transfer reverted if the transfer is still in a valid in-flight state.

### Unsigned OCW result submission calls

These calls are submitted by the offchain worker as unsigned extrinsics with signed payload verification:

- `submit_payment_result`
  Stores the result of a legacy queued payment.

- `submit_refund_result`
  Stores the result of a legacy queued refund.

- `submit_balance_result`
  Stores the latest Stripe balance snapshot.

- `submit_outbound_transfer_result`
  Applies the result of a canonical outbound transfer and drives the matching remuneration finalization or release step.

### Important internal helpers

The pallet also has a small internal helper surface that is important when reading the code:

- canonical transfer creation helpers
- guarded status update helper
- helpers to attach:
  - Stripe object id
  - external reference
  - escrow reference
  - last error
- canonical outbound Stripe idempotency key generation
- unsigned transaction `provides` key generation for OCW result deduplication

## Main Storage Objects

### Legacy queue-based storage

- `PendingPayments`
  Legacy queued Stripe payment requests awaiting OCW processing.

- `ProcessedPayments`
  Recorded results of legacy Stripe payments.

- `NextPaymentIndex`
  Monotonic identifier source for `PendingPayments` / `ProcessedPayments`.

- `PendingRefunds`
  Legacy queued Stripe refund requests awaiting OCW processing.

- `ProcessedRefunds`
  Recorded results of processed Stripe refunds.

- `NextRefundIndex`
  Monotonic identifier source for refunds.

### Bridge and balance storage

- `StripeEnabled`
  Global on/off switch for the pallet's operational behavior.

- `NextBridgeTransferId`
  Monotonic identifier source for canonical bridge transfers.

- `BridgeTransfers`
  Canonical bridge-transfer storage keyed by `bridge_id`.

- `LastBalance`
  Latest Stripe balance snapshot returned by the balance-check flow.

- `BalanceCheckRequested`
  Boolean flag indicating that a Stripe balance check should be executed by the OCW.

### Canonical `BridgeTransfer` record

`BridgeTransfer` currently stores:

- `owner`
- `amount`
- `currency`
- `direction`
- `status`
- `retry_of`
- `stripe_object_id`
- `external_reference`
- `escrow_reference`
- `last_error`

The distinction between the two reference fields is important:

- `external_reference`
  External Stripe-side or business-side reference. In the current code, this is used by the inbound flow for the Stripe/external confirmation reference.

- `escrow_reference`
  Internal remuneration escrow reference. In the current code, this is used by canonical outbound transfers and typically looks like `bridge-transfer-{id}`.

The field split is intentional: outbound escrow state and inbound external identity are no longer overloaded into one field.

## Canonical Bridge Transfer Lifecycle

### Directions

- `ToStripe`
  Outbound settlement from remuneration toward Stripe.

- `FromStripe`
  Inbound settlement from Stripe toward remuneration.

### Statuses

The enum currently contains:

- `Requested`
- `FundsReserved`
- `SubmittedToStripe`
- `AwaitingConfirmation`
- `Succeeded`
- `Failed`
- `Reverted`
- `CreditedOnChain`
- `Finalized`

Not every status is used by every direction. The transition guard is intentionally strict and direction-aware.

### Typical outbound lifecycle

The implemented outbound path follows this shape:

1. `Requested`
2. `FundsReserved`
3. `SubmittedToStripe`
4. either:
   - `Succeeded -> Finalized`
   - `Failed -> Reverted`

For admin recovery, a direct `FundsReserved -> Reverted` path is also used by force-revert.

### Typical inbound lifecycle

The trusted inbound path is intentionally shorter:

1. `Requested`
2. `CreditedOnChain`
3. `Finalized`

This is clearer than treating the transfer as still "awaiting confirmation", because the extrinsic itself already represents a trusted confirmed external event.

### Retry lineage

Retries do not recycle an old failed transfer in place.

Instead:

- the original failed or reverted transfer remains as historical state
- a retry creates a fresh canonical outbound transfer
- the new transfer records `retry_of = Some(original_bridge_id)`

This keeps audit history clearer and avoids ambiguity around escrow references and settlement state.

## Main Events

The pallet emits events in several categories.

### Operational toggle

- `StripeToggled`
  Indicates whether the bridge has been enabled or disabled.

### Legacy payment and refund lifecycle

- `StripePaymentQueued`
- `StripePaymentProcessed`
- `StripePaymentFailed`
- `StripeRefundQueued`
- `StripeRefundProcessed`

These events let developers and operators correlate legacy queue entries with Stripe-side processing results.

### Balance checks

- `BalanceCheckRequested`
- `StripeBalanceUpdated`

These indicate that a balance fetch was requested and that a snapshot has been stored on chain.

### Canonical transfer lifecycle

- `BridgeTransferCreated`
- `BridgeTransferStatusUpdated`

These are the generic canonical lifecycle events. They let an operator follow transfer creation and explicit state transitions.

### Canonical outbound flow

- `OutboundTransferToStripeRequested`
- `OutboundTransferToStripeSucceeded`
- `OutboundTransferToStripeFailed`

These summarize the higher-level operational outcome of canonical outbound settlement.

### Canonical inbound flow

- `InboundTransferFromStripeConfirmed`
- `InboundTransferFromStripeCredited`

These indicate that a trusted Stripe-side inbound confirmation was accepted and that on-chain remuneration credit was applied.

### Admin and recovery

- `OutboundTransferRetried`
- `OutboundTransferForceReverted`

These make retry lineage and recovery actions visible on chain.

## Tests and What They Validate

The test suite is fairly broad for the current scope of the pallet. It validates:

- access control through the remuneration custodian
- enabling and disabling of the bridge
- legacy payment and refund queue behavior
- canonical transfer creation and stored field correctness
- valid and invalid canonical lifecycle transitions
- outbound remuneration reserve / finalize / release behavior
- inbound exactly-once credit protection through remuneration
- retry behavior and retry lineage
- force-revert behavior for stuck outbound transfers
- uniqueness of unsigned transaction `provides` keys
- deterministic canonical outbound Stripe idempotency keys
- mocked offchain Stripe HTTP calls
- local in-flight guard behavior for canonical outbound transfers
- balance-check processing

In practice, these tests give confidence that:

- authorization is aligned with remuneration custodian control
- the canonical settlement state machine is enforced
- economic effects on remuneration match outbound and inbound results
- the offchain worker path is exercised with mocked Stripe responses
- the newer canonical flow and the older queue-based flow both still work

## Limitations and Caveats

This pallet should be read as a reasonably robust research-project bridge/orchestration pallet, not as finished production financial infrastructure.

Current caveats include:

- the Stripe integration is still testbed-oriented in parts
- weights in `src/weights.rs` are placeholder development weights, not benchmarked production weights
- the pallet depends on trusted local node configuration for offchain API keys and signing
- force-revert is an administrative recovery tool and should be used carefully
- the legacy queue-based payment/refund path still coexists with the newer canonical flow
- `AwaitingConfirmation` still exists in the status enum, but the current trusted inbound flow no longer uses it

## Developer Notes and Setup Hints

- The Stripe API key is read from offchain local persistent storage under `stripe-bridge::api-key`.
- The offchain worker must be enabled and configured in the node environment for Stripe HTTP calls to happen.
- The pallet uses an OCW signing key with key type `strp`.
- Tests mock Stripe HTTP behavior rather than talking to the live Stripe API.
- The pallet depends directly on the `remuneration` pallet interface for authorization and settlement-side helper methods.
