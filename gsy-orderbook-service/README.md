# Gsy Orderbook Service

## Getting started

+ Copy [example_configuration.yaml](example_configuration.yaml) to `configuration.yaml`
    + Edit `APPLICATION_HOST`, `APPLICATION_PORT`, `DATABASE_HOST`, `DATABASE_USERNAME`, `DATABASE_PASSWORD`, `DATABASE_NAME`, `DATABASE_URL_SCHEME`
+ run `cargo run`

## API

### `POST /orders`
```
curl -X POST http://127.0.0.1:8080/orders \
    -H "Content-Type: application/json" \
    --data-raw '[{
    "type": "Bid",
    "data": {
        "buyer": "aabc",
        "nonce": 2,
        "area_uuid": 1,
        "market_id": "0x123",
        "time_slot": 2,
        "creation_time": 1546300800,
        "bid_component": {
            "energy": 10,
            "energy_rate": 1
            }
        }
    }, {
    "type": "Offer", 
    "data": {
        "seller": "bbbcb", 
        "nonce": 1, 
        "area_uuid": 1,
        "market_id": "0x123",
        "time_slot": 1,
        "creation_time": 1546300800,
        "offer_component": {
            "energy": 10, 
            "energy_rate": 1
            }
        }
    }]'
```
### `GET /orders`
```
curl http://127.0.0.1:8080/orders
```