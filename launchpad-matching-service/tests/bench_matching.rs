//! MS-10 benchmark: matching before (`common::reference`) and after MS-10 at
//! the plan maximum request sizes (Developer 50 KiB, Base 500 KiB,
//! Advanced 10 MiB), for a realistic shape (96 quarter-hour slots, about 20
//! orders per market and slot, more markets as the batch grows) and the
//! worst case (every order in one market and one slot). A third shape, one
//! market, one slot and one area (nothing may match), is the worst case for
//! the optimized matching, which then visits every bid / offer pair.
//!
//! Ignored by default. Run it in release mode:
//!
//! ```text
//! cargo test --release --test bench_matching -- --ignored --nocapture
//! ```
//!
//! No MongoDB is needed: `/match` is called with `dry_run=true` against an
//! unreachable database. `BENCH_SKIP_OLD_ABOVE=<bytes>` skips the old
//! implementation for larger payloads (its 10 MiB worst case takes minutes).

mod common;

use actix_web::{App, HttpResponse, post, test, web};
use common::payload::{BatchShape, batch_of_size};
use common::reference::reference_match_orders;
use gsy_offchain_primitives::db_api_schema::orders::Order;
use launchpad_matching_service::api::controller::match_orders;
use launchpad_matching_service::api::types::{DbBidOfferMatch, OrdersToMatch};
use launchpad_matching_service::api::views::{self, MatchQuery};
use launchpad_matching_service::auth::jwt::{Claims, JwtSecret, encode_jwt};
use launchpad_matching_service::configuration::DEFAULT_JSON_BODY_LIMIT;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const TEST_JWT_SECRET: &str = "bench_secret";
const KIB: usize = 1024;
const MIB: usize = 1024 * 1024;

/// `POST /match` as before MS-10: same extractors and response as
/// `views::pay_as_bid` (dry run), with the reference matching.
#[post("/match")]
async fn reference_pay_as_bid(
    _claims: Claims,
    query: web::Query<MatchQuery>,
    orders: web::Json<OrdersToMatch>,
) -> HttpResponse {
    assert!(query.dry_run, "the benchmark only does dry runs");
    let (matches, _all_matches) = reference_match_orders(orders.into_inner());
    HttpResponse::Ok().json(matches)
}

/// Runs `f` at least once and until one second has passed (at most 10
/// times); returns the median duration.
fn measure<F: FnMut()>(mut f: F) -> Duration {
    let mut times = Vec::new();
    let start = Instant::now();
    while times.is_empty() || (start.elapsed() < Duration::from_secs(1) && times.len() < 10) {
        let t = Instant::now();
        f();
        times.push(t.elapsed());
    }
    times.sort();
    times[times.len() / 2]
}

macro_rules! measure_async {
    ($body:expr) => {{
        let mut times = Vec::new();
        let start = Instant::now();
        while times.is_empty() || (start.elapsed() < Duration::from_secs(1) && times.len() < 10) {
            let t = Instant::now();
            $body;
            times.push(t.elapsed());
        }
        times.sort();
        times[times.len() / 2]
    }};
}

fn ms(d: Duration) -> String {
    let millis = d.as_secs_f64() * 1000.0;
    if millis < 10.0 {
        format!("{:.2} ms", millis)
    } else {
        format!("{:.0} ms", millis)
    }
}

fn count_matches(map: &HashMap<String, Vec<DbBidOfferMatch>>) -> usize {
    map.values().map(Vec::len).sum()
}

/// (bids, offers) per (market_id, time_slot).
fn group_sizes(batch: &OrdersToMatch) -> HashMap<(String, u64), (usize, usize)> {
    let mut sizes: HashMap<(String, u64), (usize, usize)> = HashMap::new();
    for order in &batch.orders {
        match &order.order {
            Order::Bid(bid) => {
                let c = &bid.bid_component;
                sizes
                    .entry((c.market_id.clone(), c.time_slot))
                    .or_default()
                    .0 += 1;
            }
            Order::Offer(offer) => {
                let c = &offer.offer_component;
                sizes
                    .entry((c.market_id.clone(), c.time_slot))
                    .or_default()
                    .1 += 1;
            }
        }
    }
    sizes
}

#[actix_web::test]
#[ignore]
async fn bench_matching_at_plan_sizes() {
    let skip_old_above: usize = std::env::var("BENCH_SKIP_OLD_ABOVE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);
    let token = format!(
        "Bearer {}",
        encode_jwt("bench", TEST_JWT_SECRET, 1).expect("Failed to encode JWT")
    );
    let database = mongodb::Client::with_uri_str(
        "mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=200&connectTimeoutMS=200",
    )
    .await
    .expect("Failed to parse the MongoDB URI")
    .database("launchpad_matching_service_unreachable");

    let new_app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(web::Data::new(database))
            .app_data(views::json_config(DEFAULT_JSON_BODY_LIMIT))
            .app_data(views::query_config())
            .service(views::pay_as_bid),
    )
    .await;
    // The old service rejected bodies over 2 MiB (G25); the same limit as
    // the new one is used here so only the matching differs.
    let old_app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(TEST_JWT_SECRET.to_string())))
            .app_data(views::json_config(DEFAULT_JSON_BODY_LIMIT))
            .app_data(views::query_config())
            .service(reference_pay_as_bid),
    )
    .await;

    println!();
    println!(
        "| shape | body | orders | markets x slots | bids x offers (largest group) | matches | old algorithm | new algorithm | old POST | new POST |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");

    for shape in [
        BatchShape::Realistic,
        BatchShape::WorstCase,
        BatchShape::SingleArea,
    ] {
        for (label, bytes) in [
            ("50 KiB", 50 * KIB),
            ("500 KiB", 500 * KIB),
            ("10 MiB", 10 * MIB),
        ] {
            let (batch, body) = batch_of_size(shape, bytes, 42);
            let groups = group_sizes(&batch);
            let markets: HashSet<&String> = groups.keys().map(|(m, _)| m).collect();
            let slots: HashSet<u64> = groups.keys().map(|(_, s)| *s).collect();
            let largest_group = groups
                .values()
                .copied()
                .max_by_key(|(bids, offers)| bids * offers)
                .unwrap_or((0, 0));

            // Algorithm only (no JSON).
            let mut new_matches = 0;
            let new_algorithm = measure(|| {
                let result = match_orders(&batch.orders, &batch.user_id);
                new_matches = result.iter().map(|(_, m)| m.len()).sum();
            });
            let run_old = body.len() <= skip_old_above;
            let old_algorithm = if run_old {
                let mut old_matches = 0;
                let d = measure(|| {
                    let input = batch.clone();
                    let (map, _) = reference_match_orders(input);
                    old_matches = count_matches(&map);
                });
                assert_eq!(old_matches, new_matches, "old and new match counts differ");
                Some(d)
            } else {
                None
            };

            // Full in-process request: JSON parsing, matching, JSON response.
            let request = |body: Vec<u8>| {
                test::TestRequest::post()
                    .uri("/match?dry_run=true")
                    .insert_header(("Authorization", token.clone()))
                    .insert_header(("Content-Type", "application/json"))
                    .set_payload(body)
                    .to_request()
            };
            let new_post = measure_async!({
                let resp = test::call_service(&new_app, request(body.clone())).await;
                assert!(resp.status().is_success());
                let bytes = test::read_body(resp).await;
                assert!(!bytes.is_empty());
            });
            let old_post = if run_old {
                Some(measure_async!({
                    let resp = test::call_service(&old_app, request(body.clone())).await;
                    assert!(resp.status().is_success());
                    let bytes = test::read_body(resp).await;
                    assert!(!bytes.is_empty());
                }))
            } else {
                None
            };

            let show = |d: Option<Duration>| d.map(ms).unwrap_or_else(|| "skipped".to_string());
            println!(
                "| {} | {} ({} B) | {} | {} x {} | {} x {} | {} | {} | {} | {} | {} |",
                shape.name(),
                label,
                body.len(),
                batch.orders.len(),
                markets.len(),
                slots.len(),
                largest_group.0,
                largest_group.1,
                new_matches,
                show(old_algorithm),
                ms(new_algorithm),
                show(old_post),
                ms(new_post),
            );
        }
    }
}
