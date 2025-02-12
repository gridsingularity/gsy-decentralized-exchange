// use crate::algorithms::PayAsBid;
// use crate::primitives::web3::{
//     Bid, Offer, Order, OrderSchema, OrderStatus,
// };
// use anyhow::{Error, Result};
// use async_recursion::async_recursion;
// use codec::{Decode, Encode};
// use subxt_signer::sr25519::dev;
// use std::sync::{Arc, Mutex};
// use std::{thread, time};
// use subxt::{
//     SubstrateConfig,
//     OnlineClient,
//     utils::AccountId32
// };
// use tracing::{error, info};
//
//
// async fn publish_orders(
//     url: String,
//     matches: Vec<<AccountId32>>,
// ) -> Result<(), Error> {
//
//     let api = OnlineClient::<SubstrateConfig>::from_url(url).await?;
//
//     let register_order_tx = gsy_node::tx().orderbook_registry().insert_orders(matches);
//
//     let signer = dev::alice();
//     let order_submit_and_watch = api
//         .tx()
//         .sign_and_submit_then_watch_default(&register_order_tx, &signer)
//         .await?
//         .wait_for_finalized_success()
//         .await?;
//
//     let transfer_event =
//         order_submit_and_watch.find_first::<gsy_node::orderbook_registry::events::AllOrdersInserted>()?;
//
//     if let Some(event) = transfer_event {
//         info!("Orders publishing success: {event:?}");
//     } else {
//         info!("Failed to find AllOrdersInserted Event");
//     }
//
//     Ok(())
// }
