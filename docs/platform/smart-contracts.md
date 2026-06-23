# GSY DEX Smart Contracts

## Contract Set

The refactored chain layer is implemented in Solidity (`0.8.20`) and deployed by
`gsy-contracts/scripts/deploy.ts`.

### `ActorRegistry`

Purpose:

- Maps Intelligent Actor UUIDs (`bytes16`) to authorized EVM wallets.
- Supports registrar-managed wallet authorization (`registerActor`, `setActorWallet`).
- Supports actor-managed delegate/proxy authorization (`setProxy`, `isProxy`).
- Provides the authorization check used by `OrderRegistry` before accepting actor-owned order actions.

`ActorRegistry` does not hold collateral and does not expose deposit/withdraw logic. Billing and payment remain outside the DEX contract suite.

### `MarketController`

Purpose:

- Stores market open/closed state keyed by `marketId`.
- Exposes `setMarketStatus(bytes16,bool)` and `isMarketOpen(bytes16)`.
- Restricts updates to `ORCHESTRATOR_ROLE`.

### `OrderRegistry`

Purpose:

- Records order lifecycle commitments keyed by Intelligent Order UUID.
- Validates market openness before order acceptance.
- Accepts the actor wallet or an approved proxy as sender.
- Emits `OrderPlaced`, `OrderCancelled`, `OrderStatusUpdated`.

### `TradeSettlement`

Purpose:

- Validates and settles matched trades (`settleBatch`).
- Updates order statuses to executed.
- Emits all settlement data needed by off-chain storage to create a Trade object.
- Records penalties via `submitPenalties`.

`TradeSettlement` does not move funds. Billing and payment are handled by external services.

## Role Assignment at Bootstrap

Deployment script assigns:

- `ACTOR_REGISTRAR_ROLE` on `ActorRegistry` -> actor registrar signer.
- `ORCHESTRATOR_ROLE` on `MarketController` -> orchestrator signer.
- `SETTLEMENT_ROLE` on `OrderRegistry` -> `TradeSettlement`.
- `OPERATOR_ROLE` on `TradeSettlement` -> matching engine signer.
- `EXECUTION_ENGINE_ROLE` on `TradeSettlement` -> execution engine signer.

## Settlement Invariants

`settleBatch` enforces:

- Both order UUIDs are currently open.
- Submitted order data matches the canonical `OrderRegistry` data.
- Price window consistency (`bid >= clearing price >= offer`).
- Selected energy does not exceed available bid/offer energy.

If checks pass, settlement marks orders executed and emits `TradeSettled`.

## Penalty Persistence

`submitPenalties` enforces non-empty penalty entries and accumulates:

- `penaltyEnergyByTrade[tradeId]`
- `penaltyEnergyByActor[actorId]`

Off-chain execution logic checks existing on-chain penalty values to skip already submitted trades.
