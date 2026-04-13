# Docker

## Compose Topology

The default compose files start an EVM-centered local DEX environment:

1. `anvil` starts and exposes port `8545`.
2. `gsy-contracts-bootstrap` deploys contracts and grants roles.
3. Service containers start with predefined contract addresses.
4. `gsy-orderbook` subscribes to chain events and exposes APIs.

## Main Commands

```bash
# build all images
docker compose build

# run core stack
docker compose up

# run and rebuild
docker compose up --build

# stop
docker compose down
```

## Test Compose

Use the test compose file to run e2e and integration scenarios:

```bash
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit e2e-tests
```

## Important Environment Contracts

The services expect these deployed addresses:

- `MARKET_CONTROLLER_ADDRESS`
- `ORDER_REGISTRY_ADDRESS`
- `TRADE_SETTLEMENT_ADDRESS`
- `GSY_VAULT_ADDRESS`

In default local setup, they are provisioned by the bootstrap container and injected via compose env.
