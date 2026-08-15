//! End-to-end assertions that a settled trade's `TradeStatus` converges to its evaluated
//! terminal state (`Executed` or `Penalized`) once the execution engine's verdict is submitted
//! on-chain and picked up by the offchain-storage event listener.

use crate::world::MyWorld;
use cucumber::then;
use gsy_offchain_primitives::db_api_schema::trades::{TradeSchema, TradeStatus};
use gsy_offchain_primitives::utils::h256_to_string;
use std::time::Duration;
use subxt::utils::H256;
use tracing::info;

fn orderbook_url() -> String {
	std::env::var("OFFCHAIN_STORAGE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

/// GET the trade with `trade_uuid` from offchain storage at `slot`. Filters by the time window
/// only (`apply_time_window` is inclusive on both ends, so `start == end == slot` is correct) and
/// matches on the uuid, which is globally unique — no need to also filter by `market_id`.
async fn fetch_trade(world: &MyWorld, slot: u64, trade_uuid: H256) -> Option<TradeSchema> {
	let url = format!("{}/trades?start_time={}&end_time={}", orderbook_url(), slot, slot);
	let resp = world.http_client.get(url).send().await.expect("GET /trades failed");
	assert!(resp.status().is_success(), "GET /trades returned {}", resp.status());
	let trades = resp.json::<Vec<TradeSchema>>().await.expect("deserialize trades response");
	let target = h256_to_string(trade_uuid);
	trades.into_iter().find(|t| t.trade_uuid == target)
}

/// Poll `fetch_trade` until it reports `expected`, up to 40 × 5s (~200s), matching the suite's
/// established wait budgets.
async fn poll_until_status(
	world: &MyWorld,
	slot: u64,
	trade_uuid: H256,
	expected: TradeStatus,
) -> TradeSchema {
	let mut last_seen: Option<TradeStatus> = None;
	for i in 0..40 {
		info!("Waiting for trade {:?} to reach {:?}... check {}/40", trade_uuid, expected, i + 1);
		if let Some(trade) = fetch_trade(world, slot, trade_uuid).await {
			if trade.status == expected {
				return trade;
			}
			last_seen = Some(trade.status);
		}
		tokio::time::sleep(Duration::from_secs(5)).await;
	}
	panic!(
		"Timeout: trade {:?} did not reach status {:?} in offchain storage; last observed status: {}",
		trade_uuid,
		expected,
		last_seen
			.map(|s| format!("{:?}", s))
			.unwrap_or_else(|| "trade not found in storage".to_string())
	);
}

#[then("the trade is marked \"Penalized\" in the offchain storage")]
async fn verify_trade_marked_penalized(world: &mut MyWorld) {
	let trade_uuid = world
		.last_trade_uuid
		.expect("last_trade_uuid must be captured by verify_trade_on_chain before this step runs");
	let slot = world.target_delivery_time;

	// `Penalized`, not `Executed`: submit_measurements posts buyer +12.0 and seller -8.0 against
	// this 10.0 kWh trade, penalizing both the buyer (2.0 over-consumption) and the seller (2.0
	// under-production). This trade can never come out clean, so this is a positive assertion
	// that the `PenaltiesSubmitted` -> `Penalized` path ran end to end.
	poll_until_status(world, slot, trade_uuid, TradeStatus::Penalized).await;
	info!("Verified trade {:?} is marked Penalized in offchain storage", trade_uuid);
}

#[then(
	"the unpenalized PV trade is marked \"Executed\" and the penalized PV trade is marked \
	 \"Penalized\" in the offchain storage"
)]
async fn verify_pv_penalty_trade_statuses(world: &mut MyWorld) {
	let mut trades = world.pv_penalty_trades.clone();
	assert_eq!(trades.len(), 2, "two PV trades must have been captured");

	// Same waterfall order as `penalty_steps.rs::verify_pv_penalty_waterfall`: earlier commitment
	// first. `trades[0]` is the clean trade (production covers it in full), `trades[1]` is the
	// one that absorbs the 1.0 kWh shortfall.
	trades.sort_by(|a, b| a.creation_time.cmp(&b.creation_time).then_with(|| a.trade_uuid.cmp(&b.trade_uuid)));
	let earlier = trades[0].trade_uuid;
	let later = trades[1].trade_uuid;
	let slot = world.target_delivery_time;

	poll_until_both(world, slot, earlier, TradeStatus::Executed, later, TradeStatus::Penalized).await;

	// Stability re-check: sleep strictly longer than the engine's 30s polling interval so at
	// least one further evaluation cycle provably runs, then re-assert both statuses. This proves
	// the pair is a stable fixed point rather than a transient snapshot — an implementation that
	// promotes unmeasured trades to `Executed` would flip `later` back to `Executed` within one
	// such cycle.
	//
	// This asserts on a SINGLE observation on purpose. Polling until convergence here would keep
	// sampling until it happened to catch a good moment, so a status that oscillates between
	// cycles — precisely the failure this guards against — would still pass.
	tokio::time::sleep(Duration::from_secs(45)).await;
	let (earlier_status, later_status) = fetch_statuses(world, slot, earlier, later).await;
	assert_eq!(
		(earlier_status.clone(), later_status.clone()),
		(Some(TradeStatus::Executed), Some(TradeStatus::Penalized)),
		"PV trade statuses were not stable across an evaluation cycle: earlier trade {:?} was {:?}, \
		 later trade {:?} was {:?}",
		earlier,
		earlier_status,
		later,
		later_status
	);

	info!(
		"Verified stable PV trade statuses: earlier {:?} = {:?}, later {:?} = {:?}",
		earlier, earlier_status, later, later_status
	);
}

/// Read both trades' current statuses in one pass. `None` means the trade was not found.
async fn fetch_statuses(
	world: &MyWorld,
	slot: u64,
	earlier: H256,
	later: H256,
) -> (Option<TradeStatus>, Option<TradeStatus>) {
	let earlier_trade = fetch_trade(world, slot, earlier).await;
	let later_trade = fetch_trade(world, slot, later).await;
	(
		earlier_trade.map(|t| t.status),
		later_trade.map(|t| t.status),
	)
}

/// Poll until BOTH `earlier` reaches `expected_earlier` and `later` reaches `expected_later`, up
/// to 40 × 5s. Implemented as a single combined loop (rather than two sequential
/// `poll_until_status` calls) so a slow second trade cannot consume the first trade's budget, and
/// so a failure can report both observed statuses at once.
async fn poll_until_both(
	world: &MyWorld,
	slot: u64,
	earlier: H256,
	expected_earlier: TradeStatus,
	later: H256,
	expected_later: TradeStatus,
) -> (TradeStatus, TradeStatus) {
	let mut last_earlier: Option<TradeStatus> = None;
	let mut last_later: Option<TradeStatus> = None;
	for i in 0..40 {
		info!("Waiting for PV trade statuses to converge... check {}/40", i + 1);
		(last_earlier, last_later) = fetch_statuses(world, slot, earlier, later).await;
		if last_earlier == Some(expected_earlier.clone()) && last_later == Some(expected_later.clone()) {
			return (expected_earlier, expected_later);
		}
		tokio::time::sleep(Duration::from_secs(5)).await;
	}
	panic!(
		"Timeout: PV trade statuses did not converge; earlier trade {:?} expected {:?} but was {}; \
		 later trade {:?} expected {:?} but was {}",
		earlier,
		expected_earlier,
		last_earlier.map(|s| format!("{:?}", s)).unwrap_or_else(|| "not found in storage".to_string()),
		later,
		expected_later,
		last_later.map(|s| format!("{:?}", s)).unwrap_or_else(|| "not found in storage".to_string())
	);
}
