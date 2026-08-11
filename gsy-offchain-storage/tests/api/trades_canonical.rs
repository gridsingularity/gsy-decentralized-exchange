use crate::helpers::init_app;
use actix_web::web;
use gsy_offchain_primitives::db_api_schema::market::{
    AreaTopologySchema, AssetType, MarketTopologySchema,
};
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use gsy_offchain_primitives::db_api_schema::trades::{TradeParameters, TradeSchema, TradeStatus};
use serde::Deserialize;
use subxt::utils::H256;

// Mirror of the backend `TradeCanonicalSchema`: all `TradeSchema` fields are
// flattened at the top level plus the resolved name fields.
#[derive(Deserialize, Debug)]
struct TradeCanonical {
    #[serde(flatten)]
    trade: TradeSchema,
    seller_name: Option<String>,
    buyer_name: Option<String>,
}

fn create_test_order_component(area_uuid: &str) -> DbOrderComponent {
    DbOrderComponent {
        area_uuid: area_uuid.to_string(),
        market_id: "test-market".to_string(),
        time_slot: 100,
        creation_time: 1677453190,
        energy: 100.0,
        energy_rate: 10.0,
    }
}

fn create_test_trade(offer_area_uuid: &str, bid_area_uuid: &str) -> TradeSchema {
    let trade_uuid = H256::random().to_string();
    TradeSchema {
        _id: H256::random().to_string(),
        status: TradeStatus::Settled,
        seller: "shared_seller_account".to_string(),
        buyer: "shared_buyer_account".to_string(),
        market_id: "test-market".to_string(),
        time_slot: 100,
        trade_uuid: trade_uuid.clone(),
        creation_time: 1677453190,
        offer: DbOffer {
            seller: "shared_seller_account".to_string(),
            nonce: 1,
            offer_component: create_test_order_component(offer_area_uuid),
        },
        offer_hash: H256::random().to_string(),
        bid: DbBid {
            buyer: "shared_buyer_account".to_string(),
            nonce: 1,
            bid_component: create_test_order_component(bid_area_uuid),
        },
        bid_hash: H256::random().to_string(),
        residual_offer: None,
        residual_bid: None,
        parameters: TradeParameters {
            selected_energy: 14.0,
            energy_rate: 3.0,
            trade_uuid,
        },
    }
}

async fn post_test_trades(address: &str, trades: Vec<TradeSchema>) {
    let client = reqwest::Client::new();
    let resp = client
        .post(&format!("{}/trades-normalized", address))
        .header("Content-Type", "application/json")
        .json(&trades)
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());
}

#[tokio::test]
async fn get_trades_canonical_resolves_seller_and_buyer_names() {
    let app = init_app().await;
    let address = app.address;

    // The trade component `area_uuid` holds the asset's `area_hash`, so we set
    // the topology area_hash values to the values the trades will carry.
    let market = MarketTopologySchema {
        market_id: "test-market".to_string(),
        community_uuid: "community_uuid".to_string(),
        community_name: "community".to_string(),
        time_slot: 100,
        creation_time: 100,
        community_areas: vec![
            AreaTopologySchema {
                name: "pv".to_string(),
                area_uuid: "pv_uuid".to_string(),
                area_hash: "hashA".to_string(),
                area_type: AssetType::PV,
            },
            AreaTopologySchema {
                name: "meter".to_string(),
                area_uuid: "meter_uuid".to_string(),
                area_hash: "hashB".to_string(),
                area_type: AssetType::SMART_METER,
            },
        ],
    };

    let db = web::Data::new(app.db_wrapper);
    db.get_ref()
        .markets()
        .insert(market)
        .await
        .expect("Failed to insert market");

    // Resolvable trade: offer -> hashA ("pv"), bid -> hashB ("meter").
    let resolvable = create_test_trade("hashA", "hashB");
    let resolvable_id = resolvable._id.clone();
    // Unresolvable trade: component area_uuids do not match any area_hash.
    let unresolvable = create_test_trade("unknown_offer_hash", "unknown_bid_hash");
    let unresolvable_id = unresolvable._id.clone();

    post_test_trades(&address, vec![resolvable, unresolvable]).await;

    let client = reqwest::Client::new();

    // /trades-canonical carries the resolved names.
    let resp = client
        .get(&format!("{}/trades-canonical", &address))
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());
    let canonical: Vec<TradeCanonical> = resp.json().await.unwrap();
    assert_eq!(canonical.len(), 2);

    let resolved = canonical
        .iter()
        .find(|t| t.trade._id == resolvable_id)
        .expect("resolvable trade missing from response");
    assert_eq!(resolved.seller_name, Some("pv".to_string()));
    assert_eq!(resolved.buyer_name, Some("meter".to_string()));
    // Original account fields remain intact and flattened at top level.
    assert_eq!(resolved.trade.seller, "shared_seller_account");
    assert_eq!(resolved.trade.buyer, "shared_buyer_account");

    let unresolved = canonical
        .iter()
        .find(|t| t.trade._id == unresolvable_id)
        .expect("unresolvable trade missing from response");
    assert_eq!(unresolved.seller_name, None);
    assert_eq!(unresolved.buyer_name, None);

    // The existing /trades endpoint is unchanged: plain TradeSchema, no name
    // fields present in the JSON.
    let resp = client
        .get(&format!("{}/trades", &address))
        .send()
        .await
        .expect("Failed to execute request.");
    assert_eq!(200, resp.status().as_u16());
    let raw: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(raw.len(), 2);
    for trade in raw {
        assert!(trade.get("seller_name").is_none());
        assert!(trade.get("buyer_name").is_none());
    }
}
