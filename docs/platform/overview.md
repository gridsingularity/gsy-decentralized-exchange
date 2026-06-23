# GSY DEX Platform Overview

## Architecture Goal

The refactored GSY DEX architecture keeps business responsibilities split across services, while
moving blockchain logic to EVM smart contracts.  
This gives:

- Clear on-chain trust boundaries.
- Contract-enforced role-based permissions.
- Off-chain scalability for matching and execution cycles.

## Core Building Blocks

- **Target EVM chain**: local Anvil for Docker/e2e, or Energy Web Volta/EWC for remote deployment.
- **Smart contracts** (`ActorRegistry`, `MarketController`, `OrderRegistry`, `TradeSettlement`).
- **Event indexing layer** (`gsy-ethers-listener` + `gsy-offchain-storage`).
- **Business services** (orchestrator, matching engine, execution engine, community client).

## End-to-End Flow

```mermaid
flowchart LR
    MO["Market Orchestrator"] -->|setMarketStatus| MC["MarketController"]
    AR["ActorRegistry"] -->|isAuthorized| OR["OrderRegistry"]
    CC["Community Client"] -->|placeOrder| OR["OrderRegistry"]
    ME["Matching Engine"] -->|settleBatch| TS["TradeSettlement"]
    EE["Execution Engine"] -->|submitPenalties| TS
    OR -->|events| EL["gsy-ethers-listener"]
    TS -->|events| EL
    MC -->|events| EL
    EL --> OB["gsy-offchain-storage"]
```

## Runtime Interfaces

- **EVM WS endpoint**: `ws://anvil:8545` for the local contracts stack, or the configured remote RPC endpoint.
- **Off-chain storage API**: `http://gsy-offchain-storage:8080`.
- **Primary trigger model**:
  - Matching runs on block cadence.
  - Execution runs on periodic timeslot cycles.
  - Orchestrator runs time-window checks for market open/close.

## Reference Pages

- [System Components Overview](system-components-overview.md)
- [Smart Contracts](smart-contracts.md)
- [Off-Chain Storage](off-chain-storage.md)
