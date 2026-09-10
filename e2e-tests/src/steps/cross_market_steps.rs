use crate::world::gsy_node::runtime_types::gsy_primitives::orders::{
	InputBid, InputOffer, InputOrder, OrderComponent,
};
use crate::world::{gsy_node, CrossCommunity, MyWorld};
use chrono::{Duration as ChronoDuration, Utc};
use cucumber::{then, when};
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::db_api_schema::market::AssetType;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::{string_to_h256, NODE_FLOAT_SCALING_FACTOR};
use gsy_offchain_primitives::{constants::GlobalConstants, MarketType};
use std::collections::HashSet;
use std::time::Duration;
use subxt::utils::{AccountId32, H256};
use tracing::info;
const BID_RATE: f64 = 0.3;
const OFFER_RATE: f64 = 0.07;

fn community_specs() -> Vec<(&'static str, Vec<f64>, Vec<f64>)> {
	vec![
		("CrossAlpha", vec![10.0, 7.0], vec![9.0, 6.0]),
		("CrossBeta", vec![8.0, 5.0], vec![4.0, 3.0]),
	]
}

#[when("two communities each submit multiple bids and offers selected to cross-match")]
async fn build_cross_communities(world: &mut MyWorld) {
	let now = Utc::now();
	world.target_delivery_time = ((now + ChronoDuration::hours(2)).timestamp() as u64
		/ GlobalConstants.TIME_SLOT_SEC)
		* GlobalConstants.TIME_SLOT_SEC;

	let orderbook_url = std::env::var("OFFCHAIN_STORAGE_URL")
		.unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
	let adapter = AreaMarketInfoAdapter::new(Some(orderbook_url));

	let specs = community_specs();

	let external_topologies: Vec<ExternalCommunityTopology> = specs
		.iter()
		.map(|(name, bid_energies, offer_energies)| {
			let mut areas = Vec::new();
			for i in 0..bid_energies.len() {
				areas.push(ExternalAreaTopology {
					area_type: AssetType::SMART_METER,
					area_name: format!("{}_buyer_{}", name, i),
				});
			}
			for i in 0..offer_energies.len() {
				areas.push(ExternalAreaTopology {
					area_type: AssetType::PV,
					area_name: format!("{}_seller_{}", name, i),
				});
			}
			ExternalCommunityTopology { community_name: name.to_string(), areas }
		})
		.collect();

	let markets = adapter
		.get_or_create_market_topology(external_topologies, world.target_delivery_time)
		.await;
	assert_eq!(markets.len(), specs.len(), "Expected one market per community");

	world.cross_communities.clear();
	for (name, bid_energies, offer_energies) in specs.iter() {
		let market = markets
			.iter()
			.find(|m| &m.community_name == name)
			.unwrap_or_else(|| panic!("market for community {} not created", name))
			.clone();

		let area_hash = |area_name: &str| -> String {
			market
				.community_areas
				.iter()
				.find(|a| a.name == area_name)
				.unwrap_or_else(|| panic!("area {} present in topology", area_name))
				.area_hash
				.clone()
		};

		let mut bid_forecasts = Vec::new();
		for (i, energy) in bid_energies.iter().enumerate() {
			let area_name = format!("{}_buyer_{}", name, i);
			bid_forecasts.push(ForecastSchema {
				area_uuid: area_name.clone(),
				area_hash: area_hash(&area_name),
				community_uuid: market.community_uuid.clone(),
				time_slot: world.target_delivery_time,
				creation_time: 1,
				energy_kwh: *energy,
				confidence: 1.0,
			});
		}

		let mut offer_forecasts = Vec::new();
		for (i, energy) in offer_energies.iter().enumerate() {
			let area_name = format!("{}_seller_{}", name, i);
			offer_forecasts.push(ForecastSchema {
				area_uuid: area_name.clone(),
				area_hash: area_hash(&area_name),
				community_uuid: market.community_uuid.clone(),
				time_slot: world.target_delivery_time,
				creation_time: 1,
				energy_kwh: -*energy,
				confidence: 1.0,
			});
		}

		world.cross_communities.push(CrossCommunity {
			name: name.to_string(),
			market_id: string_to_h256(market.market_id.clone()),
			topology: market.clone(),
			bid_forecasts,
			offer_forecasts,
		});
	}

	info!(
		"Prepared {} communities with multiple cross-matching orders for slot {}",
		world.cross_communities.len(),
		world.target_delivery_time
	);
}

#[when("the Market Orchestrator opens the Spot markets for the cross-matching communities")]
async fn wait_for_cross_markets_to_open(world: &mut MyWorld) {
	for community in &world.cross_communities {
		assert_eq!(
			community.market_id,
			world.generate_market_id(&community.name, MarketType::Spot),
			"market_id must match the community-aware hash for {}",
			community.name
		);
	}

	let mut remaining: HashSet<H256> =
		world.cross_communities.iter().map(|c| c.market_id).collect();
	info!("Waiting for the orchestrator to open {} cross-matching markets...", remaining.len());

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	for i in 0..60 {
		if remaining.is_empty() {
			info!("✅ All cross-matching markets are open on-chain.");
			// Let the open status propagate before inserting orders.
			tokio::time::sleep(Duration::from_secs(6)).await;
			return;
		}
		info!(
			"Waiting for MarketStatusUpdated... {} market(s) remaining, check {}/60",
			remaining.len(),
			i + 1
		);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block from node")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::MarketStatusUpdated>() {
			if let Ok(e) = ev {
				if e.1 && remaining.remove(&e.0) {
					info!("-> Market opened: {:?}", e.0);
				}
			}
		}
	}
	panic!(
		"Timeout: the orchestrator did not open all cross-matching markets. Remaining: {:?}",
		remaining
	);
}

#[when("the cross-matching bids and offers are submitted for all communities")]
async fn submit_cross_orders(world: &mut MyWorld) {
	let buyer = world.users.get("charlie").unwrap().clone();
	let seller = world.users.get("bob").unwrap().clone();
	let buyer_account = AccountId32::from(buyer.public_key());
	let seller_account = AccountId32::from(seller.public_key());

	let now = Utc::now().timestamp() as u64;

	let mut bid_inputs: Vec<InputOrder<AccountId32>> = Vec::new();
	let mut offer_inputs: Vec<InputOrder<AccountId32>> = Vec::new();

	for community in &world.cross_communities {
		let market_id = community.topology.market_id.clone();
		for forecast in &community.bid_forecasts {
			bid_inputs.push(build_bid_input(
				buyer_account.clone(),
				&forecast.area_hash,
				&market_id,
				forecast.energy_kwh,
				world.target_delivery_time,
				now,
			));
		}
		for forecast in &community.offer_forecasts {
			offer_inputs.push(build_offer_input(
				seller_account.clone(),
				&forecast.area_hash,
				&market_id,
				forecast.energy_kwh,
				world.target_delivery_time,
				now,
			));
		}
	}

	let bids_tx = gsy_node::tx().orderbook_worker().insert_orders(bid_inputs);
	world
		.subxt_client
		.tx()
		.sign_and_submit_then_watch_default(&bids_tx, &buyer)
		.await
		.expect("Failed to submit bids")
		.wait_for_finalized_success()
		.await
		.expect("Bids insert_orders extrinsic failed");
	info!("Submitted all bids in a single extrinsic (signed by charlie)");

	let offers_tx = gsy_node::tx().orderbook_worker().insert_orders(offer_inputs);
	world
		.subxt_client
		.tx()
		.sign_and_submit_then_watch_default(&offers_tx, &seller)
		.await
		.expect("Failed to submit offers")
		.wait_for_finalized_success()
		.await
		.expect("Offers insert_orders extrinsic failed");
	info!("Submitted all offers in a single extrinsic (signed by bob)");
}

#[then("every settled trade pairs a bid and an offer from the same community market")]
async fn verify_no_cross_market_trades(world: &mut MyWorld) {
	let our_markets: HashSet<H256> = world.cross_communities.iter().map(|c| c.market_id).collect();

	info!(
		"Watching settled trades across {} community markets; every trade must keep its bid and offer in the same market...",
		our_markets.len()
	);

	let mut block_sub = world
		.subxt_client
		.blocks()
		.subscribe_finalized()
		.await
		.expect("Failed to subscribe to finalized blocks");

	let mut markets_with_trades: HashSet<H256> = HashSet::new();
	for i in 0..40 {
		if markets_with_trades.len() >= our_markets.len() {
			info!(
				"Observed settled trades in all {} community markets and every one stayed within its market.",
				our_markets.len()
			);
			return;
		}
		info!(
			"Waiting for OrderExecuted... {}/{} community markets have settled a same-market trade so far, check {}/40",
			markets_with_trades.len(),
			our_markets.len(),
			i + 1
		);

		let block = tokio::time::timeout(Duration::from_secs(12), block_sub.next())
			.await
			.expect("Timeout waiting for new block")
			.unwrap()
			.unwrap();

		let events = block.events().await.unwrap();
		for ev in events.find::<gsy_node::orderbook_registry::events::OrderExecuted>() {
			if let Ok(e) = ev {
				let trade = e.0;
				let bid_market = trade.bid.bid_component.market_id;
				let offer_market = trade.offer.offer_component.market_id;

				// Only consider trades belonging to this scenario's markets.
				if !our_markets.contains(&bid_market) && !our_markets.contains(&offer_market) {
					continue;
				}

				assert_eq!(
					bid_market, offer_market,
					"Cross-community match settled: the bid belongs to market {:?} but the offer to \
					 market {:?} (trade market_id {:?}). The matching engine paired orders from \
					 different community markets.",
					bid_market, offer_market, trade.market_id
				);

				markets_with_trades.insert(bid_market);
				info!(
					"Same-market trade settled in {:?} ({} energy units)",
					trade.market_id, trade.parameters.selected_energy
				);
			}
		}
	}

	assert_eq!(
		markets_with_trades.len(),
		our_markets.len(),
		"Timeout: only {} of {} community markets settled a same-market trade after 40 blocks \
		 (markets with trades: {:?}, expected: {:?})",
		markets_with_trades.len(),
		our_markets.len(),
		markets_with_trades,
		our_markets
	);
}

fn build_bid_input(
	buyer: AccountId32,
	area_hash: &str,
	market_id: &str,
	energy_kwh: f64,
	time_slot: u64,
	now: u64,
) -> InputOrder<AccountId32> {
	InputOrder::Bid(InputBid {
		buyer,
		bid_component: OrderComponent {
			area_uuid: string_to_h256(area_hash.to_string()),
			market_id: string_to_h256(market_id.to_string()),
			time_slot,
			creation_time: now,
			energy: (energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
			energy_rate: (energy_kwh.abs() * BID_RATE * NODE_FLOAT_SCALING_FACTOR) as u64,
		},
	})
}

fn build_offer_input(
	seller: AccountId32,
	area_hash: &str,
	market_id: &str,
	energy_kwh: f64,
	time_slot: u64,
	now: u64,
) -> InputOrder<AccountId32> {
	InputOrder::Offer(InputOffer {
		seller,
		offer_component: OrderComponent {
			area_uuid: string_to_h256(area_hash.to_string()),
			market_id: string_to_h256(market_id.to_string()),
			time_slot,
			creation_time: now,
			energy: (energy_kwh.abs() * NODE_FLOAT_SCALING_FACTOR) as u64,
			energy_rate: (energy_kwh.abs() * OFFER_RATE * NODE_FLOAT_SCALING_FACTOR) as u64,
		},
	})
}
