# GSY DEX Smart Contracts

## Contract Set

The refactored chain layer is implemented in Solidity (`0.8.20`) and deployed by
`gsy-contracts/scripts/deploy.ts`.

### `GsyVault`

Purpose:

- Holds user collateral.
- Supports user `deposit` and `withdraw`.
- Supports settlement-driven balance transfer (`transferBySettlement`).
- Supports account delegation (`setProxy`, `isProxy`).

### `MarketController`

Purpose:

- Stores market open/closed state keyed by `marketId`.
- Exposes `setMarketStatus(bytes32,bool)` and `isMarketOpen(bytes32)`.
- Restricts updates to `ORCHESTRATOR_ROLE`.

### `OrderRegistry`

Purpose:

- Records order lifecycle as hash commitments.
- Validates market openness before order acceptance.
- Accepts owner or approved proxy as sender.
- Emits `OrderPlaced`, `OrderCancelled`, `OrderStatusUpdated`.

### `TradeSettlement`

Purpose:

- Validates and settles matched trades (`settleBatch`).
- Updates order statuses to executed.
- Moves funds via `GsyVault.transferBySettlement`.
- Records penalties via `submitPenalties`.

## Role Assignment at Bootstrap

Deployment script assigns:

- `ORCHESTRATOR_ROLE` -> orchestrator signer.
- `SETTLEMENT_ROLE` on `OrderRegistry` -> `TradeSettlement`.
- `SETTLEMENT_ROLE` on `GsyVault` -> `TradeSettlement`.
- `OPERATOR_ROLE` on `TradeSettlement` -> matching engine signer.
- `EXECUTION_ENGINE_ROLE` on `TradeSettlement` -> execution engine signer.

## Settlement Invariants

`settleBatch` enforces:

- Both order hashes are currently open.
- Price window consistency (`bid >= clearing price >= ask`).
- Selected energy does not exceed available bid/ask energy.

If checks pass, settlement transfers collateral and marks orders executed.

## Penalty Persistence

`submitPenalties` enforces non-empty penalty entries and accumulates:

- `penaltyEnergyByTrade[tradeId]`
- `penaltyEnergyByAccount[account]`

Off-chain execution logic checks existing on-chain penalty values to skip already submitted trades.
