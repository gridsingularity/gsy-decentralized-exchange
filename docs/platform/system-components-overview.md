# GSY DEX System Components Overview

## Component Matrix

| Component | Responsibility | Primary Inputs | Primary Outputs |
|---|---|---|---|
| `gsy-contracts` | Deployable EVM contract suite | deployment config, signer keys | contract addresses, role assignments |
| `gsy-market-orchestrator` | Market open/close management | wall-clock time, market rules | `setMarketStatus` txs |
| `gsy-community-client` | Publish forecasts/measurements and orders | external topology + profile streams | Orderbook HTTP writes + `placeOrder` txs |
| `gsy-matching-engine` | Build matches and settle trades | open orders + block progression | `settleBatch` txs |
| `gsy-execution-engine` | Compute and submit penalties | settled trades + measurements | `submitPenalties` txs |
| `gsy-ethers-listener` | Contract event subscription | WS stream from EVM node | normalized callback events |
| `gsy-orderbook-service` | Off-chain API and persistence | listener callbacks + HTTP writes | query APIs for orders/trades/profiles |
| `EWDS (DDHub gateway)` | Inter-service transport and schema governance | service topics/channels | routed request/response messages |
| `mongodb` | Off-chain storage backend | service writes | persisted state |

## Deployment Topology (Docker Compose)

1. `anvil` starts and exposes `8545`.
2. `gsy-contracts-bootstrap` deploys contracts and grants roles.
3. Core services start with known contract addresses.
4. `gsy-orderbook-service` listens for events and exposes HTTP endpoints.
5. Engine services and e2e tests consume APIs and on-chain state.

When EWDS integration is enabled:

6. Service-to-service off-chain communication is routed through the DDHub client gateway.
7. Service contracts are enforced through EWDS topics, channels, and validators.

## Role and Trust Boundaries

- `MarketController.ORCHESTRATOR_ROLE` is held by orchestrator signer.
- `TradeSettlement.OPERATOR_ROLE` is held by matching engine signer.
- `TradeSettlement.EXECUTION_ENGINE_ROLE` is held by execution engine signer.
- `OrderRegistry.SETTLEMENT_ROLE` and `GsyVault.SETTLEMENT_ROLE` are granted to settlement contract.

This ensures only dedicated components can update market status, settle matches, or submit penalties.

## Data Planes

- **On-chain plane**: market status, order status transitions, settlement transfers, penalty ledger.
- **Off-chain plane**: indexed orders/trades, forecasts/measurements, analytics and querying.
- **Inter-service transport plane**: EWDS channels/topics for resilient authenticated messaging.

The event stream is the synchronization boundary between on-chain and off-chain planes.
