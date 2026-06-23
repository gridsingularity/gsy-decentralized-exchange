# Contract Deployment and Gas Reports

The contract stack is isolated from the application and e2e compose files. Run
contract deployment first, keep the target chain available, then start the GSY
services with the generated proxy addresses.

## Supported Networks

| Target | Network | RPC URL | Chain ID | Currency |
|---|---|---|---:|---|
| `local` | Local Anvil | `http://anvil:8545` inside Docker | `31337` | ETH |
| `volta` | Energy Web Volta Testnet | `https://volta-rpc.energyweb.org` | `73799` | VT |
| `ewc` | Energy Web Chain | `https://rpc.energyweb.org` | `246` | EWT |

Remote deployments require `DEPLOYER_PRIVATE_KEY`. Energy Web Chain mainnet
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
  -f docker-compose.test.yml \
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
- `MarketController` mutating calls: `setMarketStatus`.
- `OrderRegistry` mutating calls: `placeOrder`, `cancelOrder`, `updateStatus`.
- `TradeSettlement` mutating calls: `settleBatch`, `submitPenalties`.
- View-call `estimateGas` values for the read functions used by services.

View functions do not consume gas when called off-chain; they are included as
estimates for completeness.

## Committed Gas Reports

The repository keeps the latest generated Markdown gas reports as reviewable
documentation artifacts:

| Target | Report | Notes |
|---|---|---|
| Local Anvil | `contracts-output/gas-report.md` | Baseline local benchmark run. |
| Energy Web Volta Testnet | `contracts-output/volta-gas-report.md` | Remote testnet benchmark run; values depend on Volta gas price at execution time. |
| Energy Web Chain | `contracts-output/ewc-gas-report.md` | Remote mainnet benchmark run; values depend on EWC gas price at execution time. |

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
| `ACTOR_REGISTRAR_PRIVATE_KEY` | Signer granted `ACTOR_REGISTRAR_ROLE`. Defaults to the local Anvil test key for local deployments. |
| `PROXY_ADMIN_PRIVATE_KEY` | Owner signer for the generated `ProxyAdmin` contracts. Defaults to the local Anvil test key for local deployments. |
| `EWC_RPC_URL` | Energy Web Chain RPC override. Defaults to `https://rpc.energyweb.org`. |
| `VOLTA_RPC_URL` | Volta RPC override. Defaults to `https://volta-rpc.energyweb.org`. |
| `ALLOW_EWC_MAINNET_DEPLOY` | Required safety flag for EWC mainnet deployment. |
| `GAS_REPORT_ALLOW_REMOTE` | Required safety flag for gas reports on non-local networks. |
| `CONTRACTS_ENV_PATH` | Container path for generated address env file. Defaults to `/contracts/addresses.env`. |
| `GAS_REPORT_PATH` | Container path for Markdown gas report. Defaults to `/contracts/gas-report.md`. |
| `GAS_REPORT_JSON_PATH` | Container path for JSON gas report. Defaults to `/contracts/gas-report.json`. |

All `/contracts/...` paths are persisted on the host under `contracts-output/`.
