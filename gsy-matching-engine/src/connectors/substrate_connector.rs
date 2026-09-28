use anyhow::{anyhow, Error, Result};
use async_recursion::async_recursion;
use gsy_offchain_primitives::algorithms::PayAsBid;
use gsy_offchain_primitives::db_api_schema::orders::{
	DbOrderComponent, DbOrderSchema, Order as DbOrder, OrderStatus,
};
use crate::connectors::registry_filter::{
	filter_registered_open, match_order_hashes, DroppedOrderLog, FilteredOrders, RegistryStatus,
	StoredOrder,
};
use gsy_offchain_primitives::types::gsy_node::runtime_types::gsy_primitives::orders::{
	OrderReference as NodeOrderReference, OrderStatus as NodeOrderStatus,
};
use gsy_offchain_primitives::types::{
	gsy_node, Bid, BidOfferMatch, MatchingData, NodeBidOfferMatch, Offer, Order, OrderComponent,
};
use gsy_offchain_primitives::utils::{
	read_env_or, string_to_account_id, string_to_h256, NODE_FLOAT_SCALING_FACTOR,
};
use reqwest::header::{HeaderMap, HeaderValue};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{thread, time};
use subxt::backend::legacy::LegacyRpcMethods;
use subxt::backend::rpc::RpcClient;
use subxt::config::DefaultExtrinsicParamsBuilder;
use subxt::utils::H256;
use subxt::{utils::AccountId32, OnlineClient, SubstrateConfig};
use subxt_signer::sr25519::dev;
use tracing::{error, info, warn};

const MATCH_PER_NR_BLOCKS: u64 = 4;


#[async_recursion]
pub async fn substrate_subscribe(orderbook_url: String, node_url: String) -> Result<(), Error> {
	info!("Connecting to {}", node_url);

	let api = OnlineClient::<SubstrateConfig>::from_insecure_url(node_url.clone()).await?;

	let mut gsy_blocks_events = api.blocks().subscribe_finalized().await?;

	let orderbook_url = Arc::new(Mutex::new(orderbook_url));
	let node_url = Arc::new(Mutex::new(node_url.clone()));
	// Orders dropped by the registry filter that were already logged (once per order).
	let dropped_order_log = Arc::new(Mutex::new(DroppedOrderLog::new()));

	while let Some(Ok(block)) = gsy_blocks_events.next().await {
		info!("Block {:?} finalized: {:?}", block.number(), block.hash());

		let matches: Arc<Mutex<Vec<Vec<BidOfferMatch>>>> = Arc::new(Mutex::new(Vec::new()));

		if (block.number() as u64) % MATCH_PER_NR_BLOCKS == 0 {
			info!("Starting matching cycle");

			let orderbook_url_clone = Arc::clone(&orderbook_url);
			let node_url_clone = Arc::clone(&node_url);

			let matches_clone_one = Arc::clone(&matches);
			let matches_clone_two = Arc::clone(&matches_clone_one);
			let registry_api = api.clone();
			let dropped_order_log = Arc::clone(&dropped_order_log);

			if let Err(error) = tokio::task::spawn(async move {
				let orderbook_url_clone = orderbook_url_clone.lock().unwrap().to_string();

				info!("Fetching orders from {}", orderbook_url_clone.clone());

				let open_orders = fetch_open_orders_from_orderbook_service(orderbook_url_clone)
					.await
					.unwrap_or_else(|e| panic!("Failed to fetch the open orders: {:?}", e));

				let has_bid = open_orders.iter().any(|order| matches!(order.order, Order::Bid(_)));
				let has_offer =
					open_orders.iter().any(|order| matches!(order.order, Order::Offer(_)));
				if !(has_bid && has_offer) {
					info!("No open orders to match");
					return;
				}

				// Match only orders the registry holds `Open`. If the registry cannot be read,
				// skip the whole cycle rather than match unverified orders; the next cycle
				// retries.
				let FilteredOrders { bids: open_bid, offers: open_offer, dropped } =
					match filter_orders_by_registry(&registry_api, open_orders).await {
						Ok(filtered) => filtered,
						Err(e) => {
							error!(
								"Failed to read the order registry, skipping this matching cycle: {:?}",
								e
							);
							return;
						},
					};
				for order in dropped_order_log.lock().unwrap().newly_dropped(&dropped) {
					warn!(
						"Not matching order {:?} (storage _id {}) of {}: registry status {}",
						order.hash,
						order.id,
						order.owner,
						match order.status {
							Some(status) => format!("{:?}", status),
							None => "missing (not registered)".to_string(),
						}
					);
				}

				if open_bid.len() > 0 && open_offer.len() > 0 {
					info!("Open Bid - {:?}", open_bid);
					info!("Open Offer - {:?}", open_offer);

					let mut bids_by_market: HashMap<H256, Vec<Bid>> = HashMap::new();
					for bid in open_bid {
						bids_by_market
							.entry(bid.bid_component.market_id)
							.or_default()
							.push(bid);
					}

					let mut offers_by_market: HashMap<H256, Vec<Offer>> = HashMap::new();
					for offer in open_offer {
						offers_by_market
							.entry(offer.offer_component.market_id)
							.or_default()
							.push(offer);
					}

					for (market_id, bids) in bids_by_market {
						let offers = match offers_by_market.remove(&market_id) {
							Some(offers) => offers,
							None => continue,
						};

						info!(
							"Matching market {:?} with {} bid(s) and {} offer(s)",
							market_id,
							bids.len(),
							offers.len()
						);

						let mut matching_data = MatchingData { bids, offers, market_id };
						let bid_offer_matches = matching_data.pay_as_bid();
						if !bid_offer_matches.is_empty() {
							info!(
								"Market {:?} produced {} match(es)",
								market_id,
								bid_offer_matches.len()
							);
							matches_clone_one.lock().unwrap().push(bid_offer_matches);
						}
					}
				} else {
					info!("No registered open orders to match");
				}
			})
			.await
			{
				error!("Error while fetching the orderbook - {:?}", error);
			}

			let market_matches = std::mem::take(&mut *matches_clone_two.lock().unwrap());
			if !market_matches.is_empty() {
				settle_matched_orders(node_url_clone, market_matches).await;
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

/// Build a reqwest client that sends the `x-api-key` header the off-chain storage now
/// requires. The key comes from the `API_KEY` env var (default `fedecom_user`) and must
/// match the storage service's configured key.
fn authorized_storage_client() -> reqwest::Client {
	let api_key = read_env_or("API_KEY", "fedecom_user".to_string());
	let mut headers = HeaderMap::new();
	if let Ok(value) = HeaderValue::from_str(&api_key) {
		headers.insert("x-api-key", value);
	}
	reqwest::Client::builder()
		.default_headers(headers)
		.build()
		.expect("Failed to build off-chain storage HTTP client")
}

/// Fetch the `Open` orders from the off-chain storage, converted to node units, each with its
/// storage `_id`.
async fn fetch_open_orders_from_orderbook_service(url: String) -> Result<Vec<StoredOrder>, Error> {
	let res = authorized_storage_client().get(url).send().await?;
	info!("Response: {:?} {}", res.version(), res.status());
	info!("Headers: {:#?}\n", res.headers());

	let body = res.json::<Vec<DbOrderSchema>>().await?;

	Ok(body
		.into_iter()
		.filter(|order| order.status == OrderStatus::Open)
		.filter_map(|db_order_schema| match convert_db_order_to_canonical(db_order_schema.order) {
			Ok(order) => Some(StoredOrder { id: db_order_schema._id, order }),
			Err(e) => {
				error!("Failed to convert DB order to canonical: {:?}", e);
				None
			},
		})
		.collect())
}

/// Look every order up in `OrderbookRegistry.OrdersRegistry` at the latest finalized block and
/// keep those whose entry exists and is `Open` (see [`filter_registered_open`]). One storage read
/// per order, issued concurrently. Any failed read fails the whole call.
async fn filter_orders_by_registry(
	api: &OnlineClient<SubstrateConfig>,
	orders: Vec<StoredOrder>,
) -> Result<FilteredOrders, Error> {
	let storage = api.storage().at_latest().await?;
	let keys: Vec<(AccountId32, H256)> =
		orders.iter().map(|order| (order.owner().clone(), order.hash())).collect();
	let statuses = futures::future::try_join_all(keys.iter().map(|(owner, hash)| {
		let address = gsy_node::storage()
			.orderbook_registry()
			.orders_registry(NodeOrderReference { user_id: owner.clone(), hash: *hash });
		let storage = storage.clone();
		// `fetch`, never `fetch_or_default`: the map is a `ValueQuery` defaulting to `Open`, so
		// only `fetch` tells a missing entry (`None`) apart from an `Open` one.
		async move { storage.fetch(&address).await }
	}))
	.await?;
	// Keyed by the raw account bytes: `AccountId32` does not implement `Hash`.
	let statuses: HashMap<([u8; 32], H256), Option<RegistryStatus>> = keys
		.into_iter()
		.map(|(owner, hash)| (owner.0, hash))
		.zip(statuses.into_iter().map(|status| {
			status.map(|status| match status {
				NodeOrderStatus::Open => RegistryStatus::Open,
				NodeOrderStatus::Executed(_) => RegistryStatus::Executed,
				NodeOrderStatus::Deleted => RegistryStatus::Deleted,
			})
		}))
		.collect();
	Ok(filter_registered_open(orders, |owner, hash| {
		statuses.get(&(owner.0, *hash)).copied().flatten()
	}))
}

fn convert_db_order_to_canonical(order: DbOrder) -> Result<Order> {
	Ok(match order {
		DbOrder::Bid(bid) => Order::Bid(Bid {
			buyer: string_to_account_id(bid.buyer.clone())
				.ok_or_else(|| anyhow!("Invalid buyer AccountId: {}", bid.buyer))?,
			nonce: bid.nonce,
			bid_component: convert_db_order_component_to_canonical(bid.bid_component),
		}),
		DbOrder::Offer(offer) => Order::Offer(Offer {
			seller: string_to_account_id(offer.seller.clone())
				.ok_or_else(|| anyhow!("Invalid seller AccountId: {}", offer.seller))?,
			nonce: offer.nonce,
			offer_component: convert_db_order_component_to_canonical(offer.offer_component),
		}),
	})
}

fn convert_db_order_component_to_canonical(component: DbOrderComponent) -> OrderComponent {
	OrderComponent {
		area_uuid: string_to_h256(component.area_uuid),
		market_id: string_to_h256(component.market_id),
		time_slot: component.time_slot,
		creation_time: component.creation_time,
		energy: (component.energy * NODE_FLOAT_SCALING_FACTOR).round() as u64,
		energy_rate: (component.energy_rate * NODE_FLOAT_SCALING_FACTOR).round() as u64,
	}
}

async fn send_settle_trades_extrinsic(
	api: &OnlineClient<SubstrateConfig>,
	signer: &subxt_signer::sr25519::Keypair,
	nonce: u64,
	matches: Vec<NodeBidOfferMatch<AccountId32, H256>>,
) -> Result<(), Error> {
	let trade_settlement_tx = gsy_node::tx().trades_settlement().settle_trades(matches);

	let params = DefaultExtrinsicParamsBuilder::<SubstrateConfig>::new().nonce(nonce).build();
	let order_submit_and_watch = api
		.tx()
		.sign_and_submit_then_watch(&trade_settlement_tx, signer, params)
		.await?
		.wait_for_finalized_success()
		.await?;

	let transfer_event = order_submit_and_watch
		.find_first::<gsy_node::trades_settlement::events::TradesSettled>()?;

	if let Some(event) = transfer_event {
		info!("Balance transfer success: {event:?}");
	} else {
		info!("Failed to find Balances::Transfer Event");
	}

	Ok(())
}

/// The operator account's next nonce at the best block, counting its transactions still in the
/// pool (`system_accountNextIndex`), so a pending settlement is not given the same nonce again.
async fn operator_next_nonce(
	rpc: &LegacyRpcMethods<SubstrateConfig>,
	operator_account: &AccountId32,
) -> Result<u64, Error> {
	Ok(rpc.system_account_next_index(operator_account).await?)
}

async fn settle_matched_orders(
	node_url: Arc<Mutex<String>>,
	market_matches: Vec<Vec<BidOfferMatch>>,
) {
	let node_url = node_url.lock().unwrap().to_string();

	let rpc_client = match RpcClient::from_insecure_url(node_url).await {
		Ok(rpc_client) => rpc_client,
		Err(e) => {
			error!("Failed to connect to the node for settlement: {:?}", e);
			return;
		},
	};
	let api = match OnlineClient::<SubstrateConfig>::from_rpc_client(rpc_client.clone()).await {
		Ok(api) => api,
		Err(e) => {
			error!("Failed to connect to the node for settlement: {:?}", e);
			return;
		},
	};
	let rpc = LegacyRpcMethods::<SubstrateConfig>::new(rpc_client);

	let signer = dev::alice();
	let operator_account = AccountId32(signer.public_key().0);

	let mut nonce = match operator_next_nonce(&rpc, &operator_account).await {
		Ok(nonce) => nonce,
		Err(e) => {
			error!("Failed to fetch the operator account nonce: {:?}", e);
			return;
		},
	};

	for matches in market_matches {
		if matches.is_empty() {
			continue;
		}
		let market_id = matches[0].market_id;
		info!("Settling {} match(es) for market {:?}", matches.len(), market_id);
		let order_hashes = match_order_hashes(&matches);

		let transcode_bid_offer_matches: Vec<NodeBidOfferMatch<AccountId32, H256>> = matches
			.into_iter()
			.map(|bid_offer_match| -> NodeBidOfferMatch<AccountId32, H256> {
				bid_offer_match.into()
			})
			.collect();

		match send_settle_trades_extrinsic(&api, &signer, nonce, transcode_bid_offer_matches).await
		{
			Ok(()) => {
				info!("Settling trades successful for market {:?}", market_id);
				nonce += 1;
			},
			Err(e) => {
				error!(
					"Settling trades failed for market {:?} with error: {:?}; (bid hash, offer hash) of the proposal's matches: {:?}",
					market_id, e, order_hashes
				);
				// A failed send may or may not have used the nonce (rejected from the pool vs.
				// included and failed), so read it again rather than guess.
				nonce = match operator_next_nonce(&rpc, &operator_account).await {
					Ok(nonce) => nonce,
					Err(e) => {
						error!("Failed to re-read the operator account nonce: {:?}", e);
						return;
					},
				};
			},
		}
	}
}
