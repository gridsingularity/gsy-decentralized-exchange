# GSY DEX Off-Chain Storage

## Getting started

`configuration.yaml` contains local fallback defaults. Runtime environment variables override individual YAML keys, which is how Docker Compose provides container-specific values without editing this file.

For local runs, either edit `configuration.yaml` or export the required overrides:

```bash
export APPLICATION_HOST=0.0.0.0
export APPLICATION_PORT=8080
export DATABASE_HOST=localhost
export DATABASE_USERNAME=gsy
export DATABASE_PASSWORD=gsy
export DATABASE_NAME=offchain_storage
export DATABASE_URL_SCHEME=mongodb
export UPDATE_INTERVAL=1000
export EVM_NODE_URL=ws://localhost:8545
export CONTRACT_ORDER_REGISTRY=0x0000000000000000000000000000000000000000
export CONTRACT_TRADE_SETTLEMENT=0x0000000000000000000000000000000000000000
export CONTRACT_MARKET_CONTROLLER=0x0000000000000000000000000000000000000000

cargo run
```

## Run as individual service via docker compose 

To run the GSY DEX Off-Chain Storage as a separate service, the following command can be used:

```
# Run from the current directory 
docker compose -f offchain-storage-docker-compose.yml up
```

The script `populate_db_with_dummy_data.py` prepopulates the local database with fake data for testing. 

```
pip install requests pendulum
python populate_db_with_dummy_data.py
```

## API

The API of the GSY DEX Off-Chain Storage is summarized in the Postman collection file `offchain-storage-postman-collection.json`.
