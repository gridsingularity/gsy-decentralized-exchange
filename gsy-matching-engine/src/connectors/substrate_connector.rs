use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{
    DbOrderSchema, 
    OrderStatus, 
    DbBid, 
    DbOffer, 
    DbOrderComponent, 
    Order as DbOrder
};
use gsy_offchain_primitives::types::{
    Bid, 
    BidOfferMatch, 
    MatchingData, 
    Offer, 
    Order, 
    OrderComponent
};
use gsy_offchain_primitives::utils::{
    string_to_account_id, 
    string_to_h256, 
    NODE_FLOAT_SCALING_FACTOR
};
use anyhow::{anyhow, Error, Result};
use async_recursion::async_recursion;
use codec::{Decode, Encode};
use subxt_signer::sr25519::dev;
use std::sync::{Arc, Mutex};
use std::str::FromStr;
use std::{thread, time};
use subxt::{
    SubstrateConfig,
    OnlineClient,
    utils::AccountId32
};
use tracing::{error, info};

const MATCH_PER_NR_BLOCKS: u64 = 4;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod gsy_node {}

pub const DEFAULT_MARKET_ID: u8 = 1;

use crate::connectors::substrate_connector::gsy_node::runtime_types::gsy_primitives::trades::BidOfferMatch as OtherBidOfferMatch;

#[async_recursion]
pub async fn substrate_subscribe(orderbook_url: String, node_url: String) -> Result<(), Error> {
    info!("Connecting to {}", node_url);

    let api = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url.clone()).await?;

    let mut gsy_blocks_events = api.blocks().subscribe_finalized().await?;

    let orderbook_url = Arc::new(Mutex::new(orderbook_url));
    let node_url = Arc::new(Mutex::new(node_url.clone()));

    while let Some(Ok(block)) = gsy_blocks_events.next().await {
        info!("Block {:?} finalized: {:?}", block.number(), block.hash());

        let matches = Arc::new(Mutex::new(Vec::new()));

        if (block.number() as u64) % MATCH_PER_NR_BLOCKS == 0 {
            info!("Starting matching cycle");

            let orderbook_url_clone = Arc::clone(&orderbook_url);
            let node_url_clone = Arc::clone(&node_url);

            let matches_clone_one = Arc::clone(&matches);
            let matches_clone_two = Arc::clone(&matches_clone_one);

            if let Err(error) = tokio::task::spawn(async move {
                let orderbook_url_clone = orderbook_url_clone.lock().unwrap().to_string();

                info!("Fetching orders from {}", orderbook_url_clone.clone());

                let (open_bid, open_offer) =
                    fetch_open_orders_from_orderbook_service(orderbook_url_clone)
                        .await
                        .unwrap_or_else(|e| panic!("Failed to fetch the open orders: {:?}", e));

                if open_bid.len() > 0 && open_offer.len() > 0 {
                    info!("Open Bid - {:?}", open_bid);
                    info!("Open Offer - {:?}", open_offer);

                    let mut matching_data = MatchingData {
                        bids: open_bid,
                        offers: open_offer,
                        market_id: DEFAULT_MARKET_ID,
                    };
                    let bid_offer_matches = matching_data.pay_as_bid();
                    matches_clone_one.lock().unwrap().extend(bid_offer_matches);
                    info!("Matches - {:?}", matches_clone_one.lock().unwrap());
                } else {
                    info!("No open orders to match");
                }
            })
            .await
            {
                error!("Error while fetching the orderbook - {:?}", error);
            }

            if matches_clone_two.lock().unwrap().len() > 0 {
                settle_matched_orders(node_url_clone, matches_clone_two).await;
            }
        }
    }
    error!("Subscription dropped.");
    loop {
        info!("Trying to reconnect...");
        let two_seconds = time::Duration::from_millis(2000);
        thread::sleep(two_seconds);
        let orderbook_url = orderbook_url.lock().unwrap().to_string();
        let node_url = node_url.lock().unwrap().to_string();
        if let Err(error) = substrate_subscribe(orderbook_url, node_url.clone()).await {
            error!("Error - {:?}", error);
        }
    }
}

async fn fetch_open_orders_from_orderbook_service(
    url: String,
) -> Result<(Vec<Bid>, Vec<Offer>), Error> {
    let res = reqwest::get(url).await?;
    info!("Response: {:?} {}", res.version(), res.status());
    info!("Headers: {:#?}\n", res.headers());

    let body = res.json::<Vec<DbOrderSchema>>().await?;

    let open_canonical_orders: Vec<Order> = body
        .into_iter()
        .filter(|order| order.status == OrderStatus::Open)
        .filter_map(|db_order_schema| {
            match convert_db_order_to_canonical(db_order_schema.order) {
                Ok(order) => Some(order),
                Err(e) => {
                    error!("Failed to convert DB order to canonical: {:?}", e);
                    None
                }
            }
        })
        .collect();

    let mut open_bids: Vec<Bid> = Vec::new();
    let mut open_offers: Vec<Offer> = Vec::new();

    for order in open_canonical_orders {
        match order {
            Order::Bid(bid) => open_bids.push(bid),
            Order::Offer(offer) => open_offers.push(offer),
        }
    }
    
    Ok((open_bids, open_offers))
}

fn convert_db_order_to_canonical(order: DbOrder) -> Result<Order> {
    Ok(match order {
        DbOrder::Bid(bid) => Order::Bid(Bid {
            buyer: string_to_account_id(bid.buyer.clone()).ok_or_else(|| anyhow!("Invalid buyer AccountId: {}", bid.buyer))?,
            nonce: bid.nonce,
            bid_component: convert_db_order_component_to_canonical(bid.bid_component)
        }),
        DbOrder::Offer(offer) => Order::Offer(Offer {
            seller: string_to_account_id(offer.seller.clone()).ok_or_else(|| anyhow!("Invalid seller AccountId: {}", offer.seller))?,
            nonce: offer.nonce,
            offer_component: convert_db_order_component_to_canonical(offer.offer_component)
        }),
    })
}

fn convert_db_order_component_to_canonical(component: DbOrderComponent) -> OrderComponent {
    OrderComponent {
        area_uuid: string_to_h256(component.area_uuid),
        market_id: string_to_h256(component.market_id),
        time_slot: component.time_slot,
        creation_time: component.creation_time,
        energy: (component.energy * NODE_FLOAT_SCALING_FACTOR) as u64,
        energy_rate: (component.energy_rate * NODE_FLOAT_SCALING_FACTOR) as u64
    }
}

async fn send_settle_trades_extrinsic(
    url: String,
    matches: Vec<OtherBidOfferMatch<AccountId32>>,
) -> Result<(), Error> {

    let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;

    let trade_settlement_tx = gsy_node::tx().trades_settlement().settle_trades(matches);

    let signer = dev::alice();
    let order_submit_and_watch = api
        .tx()
        .sign_and_submit_then_watch_default(&trade_settlement_tx, &signer)
        .await?
        .wait_for_finalized_success()
        .await?;

    let transfer_event =
        order_submit_and_watch.find_first::<gsy_node::trades_settlement::events::TradeCleared>()?;

    if let Some(event) = transfer_event {
        info!("Balance transfer success: {event:?}");
    } else {
        info!("Failed to find Balances::Transfer Event");
    }

    Ok(())
}

async fn settle_matched_orders(node_url: Arc<Mutex<String>>, matches: Arc<Mutex<Vec<BidOfferMatch>>>) {
    tokio::task::spawn(async move {
        info!(
            "Settling following matches - {:?}",
            matches.lock().unwrap()
        );

        let node_url = node_url.lock().unwrap().to_string();
        let matches: Vec<BidOfferMatch> = matches.lock().unwrap().clone();

        let bid_offer_match_bytes = matches.encode();
        let transcode_bid_offer_matches: Vec<OtherBidOfferMatch<AccountId32>> =
            Vec::<OtherBidOfferMatch<AccountId32>>::decode(&mut &bid_offer_match_bytes[..])
                .unwrap();

        if let Ok(()) =
            send_settle_trades_extrinsic(node_url, transcode_bid_offer_matches).await
        {
            info!("Settling trades successful");
        } else {
            error!("Settling trades failed");
        }
    });
}
