import requests
from pendulum import DateTime, now
import uuid


def _random_hex_str():
    return str(uuid.uuid4().hex)

def post_market(market_id, time_slot, creation_time):
    return requests.post(url + "/market", json={
        "market_id": market_id,
        "community_uuid": "communityid_1",
        "community_name": "Community 1",
        "time_slot": time_slot,
        "creation_time": creation_time,
        "area_uuids": [
            {
                "area_uuid": "areaid_1",
                "name": "Area 1",
                "area_hash": _random_hex_str(),
            },
            {
                "area_uuid": "areaid_2",
                "name": "Area 2",
                "area_hash": _random_hex_str(),
            },
            {
                "area_uuid": "areaid_3",
                "name": "Area 3",
                "area_hash": _random_hex_str(),
            },
            {
                "area_uuid": "areaid_4",
                "name": "Area 4",
                "area_hash": _random_hex_str(),
            }
        ]
    })


def post_trade(trade_index, order_index, market_id, time_slot, creation_time, seller_index, buyer_index):
    requests.post(url + "/trades-normalized", json=[{
        "_id": "tradeId" + str(trade_index),
        "status": "Settled",
        "buyer": "Account1",
        "seller": "Account2",
        "market_id": market_id,
        "time_slot": time_slot,
        "creation_time": creation_time,
        "trade_uuid": "tradeUuid" + str(trade_index),
        "offer": {
            "seller": "Account2",
            "nonce": order_index,
            "offer_component": {
                "area_uuid": "areaid_" + str(seller_index),
                "market_id": market_id,
                "time_slot": time_slot,
                "creation_time": creation_time,
                "energy": 0.26,
                "energy_rate": 0.122
            }
        },
        "offer_hash": _random_hex_str(),
        "bid": {
            "buyer": "Account1",
            "nonce": order_index + 1,
            "bid_component": {
                "area_uuid": "areaid_" + str(buyer_index),
                "market_id": market_id,
                "time_slot": time_slot,
                "creation_time": creation_time,
                "energy": 0.26,
                "energy_rate": 0.123
            }
        },
        "bid_hash": _random_hex_str(),
        "residual_offer": None,
        "residual_bid": None,
        "parameters": {
            "selected_energy": 0.26,
            "energy_rate": 0.122,
            "trade_uuid": "tradeUuid" + str(trade_index)
        }
    }])


def post_measurements(area_uuid, time_slot, creation_time, energy):
    requests.post(url + "/measurements", json=[{
        "area_uuid": area_uuid,
        "community_uuid": "communityid_1",
        "time_slot": time_slot,
        "creation_time": creation_time,
        "energy_kwh": energy
    }])


def post_forecasts(area_uuid, time_slot, creation_time, energy):
    requests.post(url + "/forecasts", json=[{
        "area_uuid": area_uuid,
        "community_uuid": "communityid_1",
        "time_slot": time_slot,
        "creation_time": creation_time,
        "energy_kwh": energy,
        "confidence": 0.7
    }])


if __name__ == '__main__':
    url = "https://offchainst.vps.webdock.cloud"

    start_date = DateTime(year=2025, month=1, day=1)
    start_timestamp = int(start_date.timestamp())

    trade_index = 0
    order_index = 0
    for i in range(96):
        creation_time = int(now().timestamp())
        time_slot = start_timestamp + (15 * 60 * i)
        market_id = _random_hex_str()

        resp = post_market(market_id, time_slot, creation_time)
        post_trade(trade_index, order_index, market_id, time_slot, creation_time, 1, 2)
        order_index += 2
        trade_index += 1

        post_trade(trade_index, order_index, market_id, time_slot, creation_time, 3, 4)
        order_index += 2
        trade_index += 1

        post_measurements("areaid_1", time_slot, creation_time, 0.28)
        post_measurements("areaid_2", time_slot, creation_time, 1.1)
        post_measurements("areaid_3", time_slot, creation_time, 0.01)
        post_measurements("areaid_4", time_slot, creation_time, 10.2)

        post_forecasts("areaid_1", time_slot, creation_time, 0.28)
        post_forecasts("areaid_2", time_slot, creation_time, 1.1)
        post_forecasts("areaid_3", time_slot, creation_time, 0.01)
        post_forecasts("areaid_4", time_slot, creation_time, 10.2)
