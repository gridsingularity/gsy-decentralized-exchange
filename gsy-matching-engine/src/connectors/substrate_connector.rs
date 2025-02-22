use crate::algorithms::PayAsBid;
use gsy_offchain_primitives::service_to_node_schema::orders::{
    Bid, Offer, OrderStatus, Order, OrderSchema};
use crate::primitives::web3::{BidOfferMatch, MatchingData};
use crate::primitives::web3_extension::BidOfferMatch as BidOfferMatchExtension;
use anyhow::{Error, Result};
use async_recursion::async_recursion;
use codec::{Decode, Encode};
use subxt_signer::sr25519::dev;
use std::sync::{Arc, Mutex};
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

    let body = res.json::<Vec<OrderSchema>>().await?;
    let open_bid: Vec<Bid> = body
        .clone()
        .into_iter()
        .filter(|order| {
            order.status == OrderStatus::Open && matches!(order.order, Order::Bid { .. })
        })
        .map(|order| order.into())
        .collect();
    let open_offer: Vec<Offer> = body
        .into_iter()
        .filter(|order| {
            order.status == OrderStatus::Open && matches!(order.order, Order::Offer { .. })
        })
        .map(|order| order.into())
        .collect();
    Ok((open_bid, open_offer))
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

        let serialize_matches = serde_json::to_vec(&matches).unwrap();
        let bid_offer_match_extension: Vec<BidOfferMatchExtension<AccountId32>> =
            serde_json::from_slice(&serialize_matches).unwrap();
        let bid_offer_match_bytes = bid_offer_match_extension.encode();
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
