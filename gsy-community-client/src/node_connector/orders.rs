use std::str::FromStr;
use gsy_offchain_primitives::service_to_node_schema::orders::Order;
use anyhow::{Error, Result};
use subxt_signer::sr25519::dev;
use subxt::{
    SubstrateConfig,
    OnlineClient,
    utils::AccountId32
};
use tracing::info;
use crate::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::{
    InputOrder, OrderComponent, InputBid, InputOffer};

#[subxt::subxt(runtime_metadata_path = "../offchain-primitives/metadata.scale")]
pub mod gsy_node {}

pub async fn publish_orders(
    url: String,
    orders: Vec<Order>,
) -> Result<(), Error> {

    let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;

    let input_orders = orders.into_iter().map(|o| {
        match &o {
            Order::Bid(offchain_bid) => InputOrder::Bid {
                0: InputBid {
                    buyer: AccountId32::from_str(offchain_bid.buyer.clone().as_str()).unwrap(),
                    bid_component: OrderComponent {
                        area_uuid: offchain_bid.bid_component.area_uuid.clone(),
                        energy: offchain_bid.bid_component.energy.clone(),
                        energy_rate: offchain_bid.bid_component.energy_rate.clone(),
                        market_uuid: offchain_bid.bid_component.market_uuid.clone(),
                        creation_time: offchain_bid.bid_component.creation_time.clone(),
                        time_slot: offchain_bid.bid_component.time_slot.clone(),
                    }
                }
            },
            Order::Offer(offchain_offer) => InputOrder::Offer {
                0: InputOffer {
                    seller: AccountId32::from_str(offchain_offer.seller.clone().as_str()).unwrap(),
                    offer_component: OrderComponent {
                        area_uuid: offchain_offer.offer_component.area_uuid.clone(),
                        energy: offchain_offer.offer_component.energy.clone(),
                        energy_rate: offchain_offer.offer_component.energy_rate.clone(),
                        market_uuid: offchain_offer.offer_component.market_uuid.clone(),
                        creation_time: offchain_offer.offer_component.creation_time.clone(),
                        time_slot: offchain_offer.offer_component.time_slot.clone(),
                    }
                }

            }
        }
    }).collect();
    let register_order_tx = gsy_node::tx().orderbook_worker().insert_orders(input_orders);

    let signer = dev::alice();
    let order_submit_and_watch = api
        .tx()
        .sign_and_submit_then_watch_default(&register_order_tx, &signer)
        .await?
        .wait_for_finalized_success()
        .await?;

    let transfer_event =
        order_submit_and_watch.find_first::<gsy_node::orderbook_registry::events::AllOrdersInserted>()?;

    if let Some(event) = transfer_event {
        info!("Orders publishing success: {event:?}");
    } else {
        info!("Failed to find AllOrdersInserted Event");
    }

    Ok(())
}
