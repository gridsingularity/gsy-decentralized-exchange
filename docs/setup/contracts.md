# Contract Deployment and Gas Reports

The contract stack is isolated from the application and e2e compose files. Run
contract deployment first, keep the target chain available, then start the GSY
services with the generated proxy addresses.

## Fresh Deployments

The DD-453 trade lifecycle targets fresh contract deployments and an empty
off-chain database. Start the listener before submitting orders. No migration
or reindexing tool is provided for older lifecycle records, and the listener
does not persist its replay cursor across restarts.

## Supported Networks

| Target | Network | RPC URL | Chain ID | Currency |
|---|---|---|---:|---|
| `local` | Local Anvil | `http://anvil:8545` inside Docker | `31337` | ETH |
| `volta` | Energy Web Volta Testnet | `https://volta-rpc.energyweb.org` | `73799` | VT |
| `ewc` | Energy Web Chain | `https://rpc.energyweb.org` | `246` | EWT |

Remote deployments require `DEPLOYER_PRIVATE_KEY` and an explicit
`COMMUNITY_CLIENT_PRIVATE_KEY`. Energy Web Chain mainnet
deployment also requires the explicit safety flag
`ALLOW_EWC_MAINNET_DEPLOY=true`.

## Local Deployment

Deploy contracts to a dedicated local Anvil container:

```bash
./scripts/contracts.sh local deploy
```

This command:

- Starts `anvil` from `docker-compose.contracts.yml` and leaves it running.
- Deploys the upgradeable contract suite.
- Grants the runtime roles.
- Writes Docker-compatible addresses to `contracts-output/addresses.env`.

Start the application stack against those deployed addresses:

```bash
docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.yml \
  up --build
```

Run e2e tests against the same deployed local chain:

```bash
docker compose --env-file contracts-output/addresses.env \
  -f docker-compose.e2e-test.yml \
  up --build --force-recreate \
  --abort-on-container-exit \
  --exit-code-from e2e-tests \
  e2e-tests
```

Stop the local contracts chain when it is no longer needed:

```bash
docker compose -f docker-compose.contracts.yml \
  --profile local-contracts \
  down --remove-orphans
```

Add `-v` only when you want to wipe the local Anvil state as well.

## Community-Client Authorization

Deployment grants `OrderRegistry.COMMUNITY_CLIENT_ROLE` to the wallet derived
from `COMMUNITY_CLIENT_PRIVATE_KEY` (the deployer if unset on local chains only,
chain IDs 31337 or 1337). Non-local deployments fail before deploying contracts
if this key is missing. Use the same key for
the corresponding community-client service. DD-457 requires a fresh deployment,
not an in-place upgrade of the old `OrderRegistry` storage layout.

This bootstrap signer is not a single-client limit. The registry administrator
can call `grantRole(COMMUNITY_CLIENT_ROLE, clientAddress)` for each additional
client and `revokeRole(COMMUNITY_CLIENT_ROLE, clientAddress)` to remove access;
`hasRole` checks current authorization. Revoking one client does not affect others.
Both placement and cancellation require this role, including for administrators.
The role is global across communities: authorized clients are trusted to validate
facility/community eligibility off-chain. Actor wallets and delegates have no
implicit submission permission. Matching and execution roles remain separate.

## Local Gas Report

Generate the local gas report:

```bash
./scripts/contracts.sh local gas-report
```

Outputs:

- `contracts-output/gas-report.md`
- `contracts-output/gas-report.json`

The report deploys a benchmark-only contract suite and records gas for:

- Implementation deployments.
- Proxy, `ProxyAdmin`, and initializer deployments.
- Role grants.
- `ActorRegistry` mutating calls: `registerActor`, `setActorWallet`,
  `setProxy`.
- `MarketController` mutating calls: `createMarkets` (1, 3 and 50 markets,
  and resending existing markets).
- `OrderRegistry` mutating calls: `placeOrder`, `cancelOrder`, `updateStatus`,
  and direct `settleOrder` partial-fill/residual-consumption benchmarks.
- `TradeSettlement` mutating calls: `settleBatch`, `submitPenalties`,
  `completeDeliveryVerification` (zero penalties and a new seller penalty on
  separate settled trades; prerequisite transactions are reported separately).
- View-call `estimateGas` values for the read functions used by services.

View functions do not consume gas when called off-chain; they are included as
estimates for completeness.

Settlement is measured for standard matches and buyer-only, seller-only and
reciprocal preferred matches, at batch sizes `1,2,5` by default. Override them
through Compose with, for example:

```bash
GAS_REPORT_SETTLE_BATCH_SIZES=1,2,5,10 ./scripts/contracts.sh local gas-report
```

Each match consumes a 100000 bid against a 150000 offer at price 12000 and
registers a 50000 residual offer. Preferred cases use equal effective rates;
the non-preferring side in a one-sided case falls back to its normal rate.
All cases use unique orders; prerequisite placements are recorded separately.
Batch-size comparisons include storage and calldata costs, not only match-type
validation. Direct `settleOrder` rows are isolated benchmarks: production
`settleBatch` measurements already include those internal calls.

The script deploys a separate benchmark suite and does not update
`addresses.env`. Report totals include fixture preparation and every alternative
benchmark, not one production workflow. Local gas fees are not EWC/Volta quotes.

Benchmark markets open at the latest chain timestamp and close roughly 24 hours
later so remote runs can cross quarter-hour boundaries. The script manages each
signer's transaction nonces locally; do not use the same signer concurrently in
another process. It stops on submission errors rather than automatically replacing
or resending transactions.

## Committed Gas Reports

The repository keeps the latest generated Markdown gas reports as reviewable
documentation artifacts:

| Target | Report | Notes |
|---|---|---|
| Local Anvil | `contracts-output/gas-report.md` | Baseline local benchmark run. |
| Energy Web Volta Testnet | `contracts-output/volta-gas-report.md` | Historical snapshot predating DD-442; not updated for match-type validation or atomic residual settlement. |
| Energy Web Chain | `contracts-output/ewc-gas-report.md` | Historical snapshot predating DD-442; not updated for match-type validation or atomic residual settlement. |

Generated address files and JSON reports are intentionally left untracked. They
remain useful locally, but they are environment-specific execution artifacts
rather than stable documentation.

## Volta Deployment

Deploy to Energy Web Volta Testnet:

```bash
DEPLOYER_PRIVATE_KEY=0x... \
CONTRACTS_ENV_PATH=/contracts/volta-addresses.env \
./scripts/contracts.sh volta deploy
```

Generate a Volta gas report:

```bash
DEPLOYER_PRIVATE_KEY=0x... \
GAS_REPORT_ALLOW_REMOTE=true \
GAS_REPORT_PATH=/contracts/volta-gas-report.md \
GAS_REPORT_JSON_PATH=/contracts/volta-gas-report.json \
./scripts/contracts.sh volta gas-report
```

Remote gas reports deploy contracts and send state-changing transactions. They
consume real VT on Volta.

## Energy Web Chain Mainnet Deployment

Deploy to Energy Web Chain mainnet:

```bash
DEPLOYER_PRIVATE_KEY=0x... \
ALLOW_EWC_MAINNET_DEPLOY=true \
CONTRACTS_ENV_PATH=/contracts/ewc-addresses.env \
./scripts/contracts.sh ewc deploy
```

Generate an EWC gas report:

```bash
DEPLOYER_PRIVATE_KEY=0x... \
ALLOW_EWC_MAINNET_DEPLOY=true \
GAS_REPORT_ALLOW_REMOTE=true \
GAS_REPORT_PATH=/contracts/ewc-gas-report.md \
GAS_REPORT_JSON_PATH=/contracts/ewc-gas-report.json \
./scripts/contracts.sh ewc gas-report
```

Remote gas reports deploy contracts and send state-changing transactions. They
consume real EWT on Energy Web Chain.

## Environment Variables

| Variable | Purpose |
|---|---|
| `DEPLOYER_PRIVATE_KEY` | Remote deployer signer. Also used by Hardhat for remote accounts. |
| `ORCHESTRATOR_SIGNER_PRIVATE_KEY` | Signer granted `ORCHESTRATOR_ROLE`. Defaults to the local Anvil test key for local deployments. |
| `MATCHING_ENGINE_PRIVATE_KEY` | Signer granted `OPERATOR_ROLE`. Defaults to the local Anvil test key for local deployments. |
| `EXECUTION_ENGINE_PRIVATE_KEY` | Signer granted `EXECUTION_ENGINE_ROLE`. Defaults to the local Anvil test key for local deployments. |
| `COMMUNITY_CLIENT_PRIVATE_KEY` | Bootstrap signer granted `OrderRegistry.COMMUNITY_CLIENT_ROLE`; use the same key for the client service. Defaults to the local Anvil test key locally, or to the deployer when unset. Additional clients are authorized through `grantRole`. |
| `ACTOR_REGISTRAR_PRIVATE_KEY` | Signer granted `ACTOR_REGISTRAR_ROLE`. Defaults to the local Anvil test key for local deployments. |
| `PROXY_ADMIN_PRIVATE_KEY` | Owner signer for the generated `ProxyAdmin` contracts. Defaults to the local Anvil test key for local deployments. |
| `EWC_RPC_URL` | Energy Web Chain RPC override. Defaults to `https://rpc.energyweb.org`. |
| `VOLTA_RPC_URL` | Volta RPC override. Defaults to `https://volta-rpc.energyweb.org`. |
| `ALLOW_EWC_MAINNET_DEPLOY` | Required safety flag for EWC mainnet deployment. |
| `GAS_REPORT_ALLOW_REMOTE` | Required safety flag for gas reports on non-local networks. |
| `CONTRACTS_ENV_PATH` | Container path for generated address env file. Defaults to `/contracts/addresses.env`. |
| `GAS_REPORT_PATH` | Container path for Markdown gas report. Defaults to `/contracts/gas-report.md`. |
| `GAS_REPORT_JSON_PATH` | Container path for JSON gas report. Defaults to `/contracts/gas-report.json`. |
| `GAS_REPORT_SETTLE_BATCH_SIZES` | Comma-separated positive integers for standard and preferred batch benchmarks. Defaults to `1,2,5`. |

All `/contracts/...` paths are persisted on the host under `contracts-output/`.
