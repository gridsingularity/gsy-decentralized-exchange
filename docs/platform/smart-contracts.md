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

- Stores one record per `marketId` that mirrors the off-chain `MarketSchema`:
  community ID, opening/closing time, delivery start/end, market type,
  matching algorithm and `createdAt` (block timestamp; `0` means missing).
- `createMarkets(NewMarket[])` (`ORCHESTRATOR_ROLE` only) stores new markets
  and emits `NewMarketCreated` with every field. Existing markets are skipped
  without an event. `InvalidMarket` is raised for a zero market or community
  ID, `closingTime <= openingTime` or `deliveryEndTime <= deliveryStartTime`;
  one invalid entry reverts the whole batch.
- There is no open/closed flag and no update function: `isMarketOpen(bytes16)`
  is true while `openingTime <= block.timestamp < closingTime`.
- Views: `marketsExist(bytes16[])` (existence of many markets in one call,
  used by the orchestrator) and `isMarketOpen(bytes16)` (used by
  `OrderRegistry`). The full record is read from `NewMarketCreated` or from
  off-chain storage; there is no record getter.
- Enum values: `MarketType` `Spot=0, Flex=1, Settlement=2`;
  `MatchingAlgorithm` `PayAsBid=0, PayAsClear=1, Amm=2` (same order as
  `primitives`).

### `OrderRegistry`

Purpose:

- Records order lifecycle commitments keyed by Intelligent Order UUID.
- Validates market openness before order acceptance.
- Accepts the actor wallet or an approved proxy as sender.
- Stores order requirements (bids and offers, including the preferred trading
  partner) and offer attributes used by matching.
- Emits the complete order metadata in `OrderPlaced`, allowing the event
  listener to reconstruct the off-chain order without a separate update.
- Emits `OrderCancelled` and `OrderStatusUpdated` lifecycle events.
- `settleOrder` consumes an open order and registers any residual atomically,
  copying stored metadata and reducing only its energy. It requires `SETTLEMENT_ROLE`.
- Both `getStatus` and `orderStatus` derive `Expired` for an open order once its
  market closes. This includes residuals. Settlement and cancellation reject
  these orders; consumed statuses remain unchanged. Market windows are immutable,
  so recreating an existing market ID cannot reactivate expired orders.
- Expiry requires no per-order transaction or `OrderStatusUpdated` event. Storage
  derives it from confirmed block timestamps and the indexed `NewMarketCreated`
  closing time, after indexing the block's contract events.

DD-439 replaces the old boolean market mapping with full market records. This is
not a storage-compatible upgrade of the old `MarketController`. Use a fresh,
coordinated deployment and matching contract addresses for all services, and
rebuild the corresponding off-chain market projection. Do not upgrade an existing
boolean-mapping deployment in place without a separately designed migration.

### `TradeSettlement`

Purpose:

- Validates and settles matched trades (`settleBatch`).
- Marks consumed orders `Filled` or `PartiallyFilled` and registers residuals as
  separate `Open` orders. Existing status enum values are preserved; the appended
  values are `PartiallyFilled = 5` and `Filled = 6`.
- Emits all settlement data needed by off-chain storage to create a Trade object.
- Records penalties via `submitPenalties`.
- Stores the consumed order IDs for each new settlement; trade IDs cannot be reused.
- Accepts idempotent `completeDeliveryVerification` batches from
  `EXECUTION_ENGINE_ROLE`. Penalties and `Executed` order statuses are recorded
  in the same transaction, followed by `DeliveryVerified(tradeId,bidId,offerId)`.
  It trusts the engine's off-chain measurement verdict; no meter data is verified
  inside the contract. Residual orders are not completed by their parent's verdict.

Verification storage is appended without changing existing proxy slots. Upgrade
`OrderRegistry` before `TradeSettlement`, then deploy the updated execution engine
and listener. Trades settled before this upgrade have no stored order-ID linkage
and cannot use the new completion call without a separate audited migration.
No historical records or verdicts are fabricated/backfilled automatically.

`TradeSettlement` does not move funds. Billing and payment are handled by external services.

### Shared Order Parameters

`OrderRegistry.OrderParams` is the single Solidity order definition used by
`placeOrder`, `getOrder`, and both orders in `TradeSettlement.Match`. It includes
`isBid`, `preferredTradingPartner` and `preferredEnergyRate` alongside the
identity, energy, and timing fields. There is no separate
`TradeSettlement.OrderData` definition.

Settlement verifies these fields against the stored orders and requires
`isBid=true` for the bid and `isBid=false` for the offer. Contextual partner and
price validation additionally depends on `Match.matchType`.

### Match Type and Compatibility

`Match` ends with `MatchType matchType`: `Standard = 0`, `Preferred = 1`.
Adding this field changes the `settleBatch` calldata ABI and function selector.
The matcher and every direct caller must use the matching contract ABI; old
calldata is not compatible. The flag is not added to `TradeSettled` or storage.
It describes the submitted match, not the residual order's eligibility in a
later phase.

`settleBatch` accepts `MarketSettlement[]`: each entry contains `Match[] matches`
and a `ClearingResult`. The contract checks that `tradedQuantity` equals the sum
of selected energy in that entry and emits `MarketClearing` after its trades.
Callers must include this per-market envelope as well as each match's type.

Deploy or upgrade the registry and settlement implementations together with the
updated matcher. For local validation, run `./scripts/contracts.sh local deploy`
and restart services using the refreshed `contracts-output/addresses.env`.

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
- `Standard`: normal price limits hold (`bid >= clearing price >= offer`),
  even when an order contains partner preferences.
- `Preferred`: at least one partner is specified; every specified partner
  equals the opposite order's `createdBy`. Effective price limits hold
  (`effective offer <= clearing price <= effective bid`). An effective rate is
  `preferredEnergyRate` when non-zero, otherwise `energyRate`. Normal price
  limits do not apply to this match type.
  The matcher selects the effective bid rate; the contract validates the
  inclusive bounds rather than enforcing that pricing choice.
- Selected energy is positive and does not exceed either order's energy.
- Each partially filled order has a fresh, non-zero residual ID; a fully filled
  order has no residual. Residual quantity and metadata come from the registry.

If checks pass, settlement marks consumed orders `Filled`/`PartiallyFilled` and
emits `OrderPlaced` for their residuals and `TradeSettled` for the trade.
`Executed` is reserved for subsequent delivery verification. Later matches may
consume residuals within the same batch or a subsequent transaction.
An invalid match reverts the entire batch, including residual creation.

The operator supplies the match type. These checks validate the claimed layer's
rules, not global preference priority or optimal matching across the order book.

## Penalty Persistence

`submitPenalties` enforces non-empty penalty entries and accumulates:

- `penaltyEnergyByTrade[tradeId]`
- `penaltyEnergyByActor[actorId]`
- `penaltyEnergyByTradeAndActor[tradeId][actorId]`

The legacy penalty call remains available but cannot add penalties to verified
trades. The execution engine now uses atomic verification instead. Verification
reuses an exactly matching per-actor penalty, rejects conflicting totals, and
never records penalties twice for a repeated verdict.
