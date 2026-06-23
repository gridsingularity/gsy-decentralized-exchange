# GSY DEX Smart Contracts

## Contract Set

The chain layer uses Solidity `^0.8.22` and is deployed by
`gsy-contracts/scripts/deploy.ts`. Hardhat compiles with `evmVersion: "paris"`
to keep bytecode compatible with the local/EWC-style runtime assumptions.

## Upgrade Strategy

The contract suite uses the OpenZeppelin transparent proxy pattern:

- Each business contract is deployed as an implementation contract.
- Each runtime address used by services is a `TransparentUpgradeableProxy`.
- Each proxy is controlled by a `ProxyAdmin` contract.
- `PROXY_ADMIN_PRIVATE_KEY` controls the `ProxyAdmin` ownership during deployment.
- Services must always use the proxy addresses (`ACTOR_REGISTRY_ADDRESS`,
  `MARKET_CONTROLLER_ADDRESS`, `ORDER_REGISTRY_ADDRESS`, `TRADE_SETTLEMENT_ADDRESS`),
  not the implementation addresses.

Implementations use OpenZeppelin upgradeable base contracts, `initialize(...)`
functions instead of constructors, and implementation contracts disable direct
initialization in their constructors. The deployment script exports both proxy
addresses and implementation/admin addresses to `/contracts/addresses.env` for
inspection and future upgrade operations.

Local Docker deployment writes the same values to
`contracts-output/addresses.env` on the host:

```bash
./scripts/contracts.sh local deploy
```

Use that generated file when starting the services or e2e tests:

```bash
docker compose --env-file contracts-output/addresses.env up --build
```

Remote deployments are supported through the dedicated contracts compose stack:

```bash
DEPLOYER_PRIVATE_KEY=0x... ./scripts/contracts.sh volta deploy

DEPLOYER_PRIVATE_KEY=0x... \
ALLOW_EWC_MAINNET_DEPLOY=true \
./scripts/contracts.sh ewc deploy
```

See [Contract Deployment and Gas Reports](../setup/contracts.md) for the full
network matrix and safety flags.

## Gas Reporting

`gsy-contracts/scripts/gas-report.ts` deploys a benchmark contract suite and
records gas for deployment, proxy initialization, role setup, mutating contract
calls, and view-call estimates.

Local report:

```bash
./scripts/contracts.sh local gas-report
```

Outputs:

- `contracts-output/gas-report.md`
- `contracts-output/gas-report.json`

Remote gas reports require an explicit opt-in because they deploy contracts and
send state-changing transactions on the target network:

```bash
DEPLOYER_PRIVATE_KEY=0x... \
GAS_REPORT_ALLOW_REMOTE=true \
./scripts/contracts.sh volta gas-report
```

Generic upgrade command:

```bash
UPGRADE_CONTRACT_NAME=ActorRegistry \
UPGRADE_PROXY_ADDRESS="$ACTOR_REGISTRY_ADDRESS" \
UPGRADE_PROXY_ADMIN_ADDRESS="$ACTOR_REGISTRY_PROXY_ADMIN_ADDRESS" \
PROXY_ADMIN_PRIVATE_KEY="$PROXY_ADMIN_PRIVATE_KEY" \
npx hardhat run scripts/upgrade.ts --network anvil
```

Set `UPGRADE_CALL_DATA` when an upgrade needs a post-upgrade initializer or
migration call; otherwise the script uses `0x`.

Future implementations must preserve storage layout: do not reorder, remove, or
change existing state variable types; append new storage only after existing
state variables.

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

- Proxy admin ownership -> proxy admin owner signer.
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
