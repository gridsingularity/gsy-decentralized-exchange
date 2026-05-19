# GSY DEX Off-Chain Storage

## Getting started

+ Copy [example_configuration.yaml](example_configuration.yaml) to `configuration.yaml`
    + Edit `APPLICATION_HOST`, `APPLICATION_PORT`, `DATABASE_HOST`, `DATABASE_USERNAME`, `DATABASE_PASSWORD`, `DATABASE_NAME`, `DATABASE_URL_SCHEME`
+ run `cargo run`

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