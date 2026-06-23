# Run

## Recommended: Full Stack with Docker Compose

```bash
./scripts/contracts.sh local deploy

docker compose --env-file contracts-output/addresses.env up --build
```

The contracts command starts local Anvil, deploys the upgradeable contract
suite, grants roles, and writes `contracts-output/addresses.env`. Keep that
Anvil container running while the service stack is active.

The service compose brings up:

- `mongodb`
- `gsy-offchain-storage`
- `gsy-market-orchestrator`
- `gsy-matching-engine`
- `gsy-execution-engine`
- `gsy-community-client`

## Core Endpoints

- EVM RPC: `http://localhost:8545` (WS available on same port)
- Off-chain storage API: `http://localhost:8080`
- Health check: `http://localhost:8080/health_check`

## Stop Stack

```bash
docker compose down
```

Stop the local contracts chain separately:

```bash
docker compose -f docker-compose.contracts.yml \
  --profile local-contracts \
  down --remove-orphans
```

To remove named volumes as well:

```bash
docker compose down -v
```
