# Run

## Recommended: Full Stack with Docker Compose

```bash
docker compose up --build
```

This brings up:

- `anvil`
- `gsy-contracts-bootstrap`
- `mongodb`
- `gsy-orderbook`
- `gsy-market-orchestrator`
- `gsy-matching-engine`
- `gsy-execution-engine`
- `gsy-community-client`

## Core Endpoints

- EVM RPC: `http://localhost:8545` (WS available on same port)
- Orderbook API: `http://localhost:8080`
- Health check: `http://localhost:8080/health_check`

## Stop Stack

```bash
docker compose down
```

To remove named volumes as well:

```bash
docker compose down -v
```
