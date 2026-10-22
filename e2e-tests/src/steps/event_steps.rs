use crate::steps::trade_steps::{
    align_to_matching_window, market_id_as_hex, mine_until_matching_block, query_orders_for_market,
    wait_for_order_in_offchain_storage, OrderRegistryContract,
};
use crate::world::MyWorld;
use cucumber::{given, then, when};
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use primitives::db_api_schema::grid_topology::{FacilitySchema, SiteSchema};
use primitives::db_api_schema::profiles::MeasurementSchema;
use primitives::db_api_schema::trades::TradeStatus;
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsMeasurementDto, EwdsOrderDto, EwdsTradeDto};
use primitives::ewds::{EwdsClient, EwdsClientConfig, EwdsEventType};
use primitives::utils::{bytes16_to_hex, epoch_to_rfc3339, parse_uuid_or_hex_bytes16};
use std::time::Duration;
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{sleep, timeout},
};
use uuid::Uuid;

const EVENT_POLL_ATTEMPTS: usize = 60;

#[derive(Debug)]
pub struct TradeStatusObserver {
    worker: JoinHandle<()>,
    receiver: mpsc::UnboundedReceiver<(EwdsTradeDto, String)>,
    received: Vec<(EwdsTradeDto, String)>,
}

impl Drop for TradeStatusObserver {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

#[given("FOS listens for trade status notifications")]
async fn observe_trade_statuses(world: &mut MyWorld) {
    let mut config = EwdsClientConfig::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000);
    config.consumer_client_id = format!("gsye2etrades{}", Uuid::new_v4().simple());
    let client = EwdsClient::new(config);
    let (sender, receiver) = mpsc::unbounded_channel();
    let worker = tokio::spawn(async move {
        client
            .run_event_worker(EwdsEventType::Trade, |event| {
                let sender = sender.clone();
                async move {
                    for trade in serde_json::from_value::<Vec<EwdsTradeDto>>(event.data)? {
                        sender.send((trade, event.occurred_at.clone()))?;
                    }
                    Ok(())
                }
            })
            .await;
    });
    world.trade_status_observer = Some(TradeStatusObserver {
        worker,
        receiver,
        received: vec![],
    });
}

#[then(expr = "FOS receives the {word} trade notification")]
async fn verify_trade_notification(world: &mut MyWorld, status: String) {
    let status = match status.as_str() {
        "Matched" => TradeStatus::Matched,
        "Settled" => TradeStatus::Settled,
        "Executed" => TradeStatus::Executed,
        "Penalized" => TradeStatus::Penalized,
        _ => panic!("Unexpected trade status: {status}"),
    };
    let trade = world.last_trade.as_ref().expect("Missing settled trade");
    let occurred_at = super::lifecycle_steps::trade_status_timestamp(world, trade, &status).await;
    let mut expected = trade.clone();
    expected.status = status;
    let expected = EwdsTradeDto::from(expected);
    let observer = world
        .trade_status_observer
        .as_mut()
        .expect("FOS is not listening for trades");
    timeout(Duration::from_secs(180), async {
        loop {
            // Background publication can reorder or duplicate notifications. Match by ID/status.
            if let Some((actual, timestamp)) = observer.received.iter().find(|(trade, _)| {
                trade.trade_id == expected.trade_id && trade.trade_status == expected.trade_status
            }) {
                assert_eq!(
                    actual, &expected,
                    "Lifecycle notification changed trade details"
                );
                assert_eq!(timestamp, &epoch_to_rfc3339(occurred_at));
                return;
            }
            observer.received.push(
                observer
                    .receiver
                    .recv()
                    .await
                    .expect("Trade observer stopped"),
            );
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "Timeout: no {} notification for trade {}",
            expected.trade_status, expected.trade_id,
        )
    });
}

#[derive(Debug)]
pub struct OrderStatusObserver {
    worker: JoinHandle<()>,
    receiver: mpsc::UnboundedReceiver<EwdsOrderDto>,
    received: Vec<EwdsOrderDto>,
}

impl Drop for OrderStatusObserver {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

impl OrderStatusObserver {
    async fn wait_for(&mut self, id: &str, status: &str) -> EwdsOrderDto {
        let id = parse_uuid_or_hex_bytes16(id).unwrap();
        timeout(Duration::from_secs(180), async {
            loop {
                if let Some(order) = self.received.iter().find(|order| {
                    parse_uuid_or_hex_bytes16(&order.order_id) == Some(id)
                        && order.order_status == status
                }) {
                    assert!(
                        order.updated_at.is_some(),
                        "Status notification lacks updatedAt"
                    );
                    return order.clone();
                }
                self.received
                    .push(self.receiver.recv().await.expect("Status observer stopped"));
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "Timeout: no {status} notification for {}",
                bytes16_to_hex(id)
            )
        })
    }
}

#[given("FOS listens for order status notifications")]
async fn observe_order_statuses(world: &mut MyWorld) {
    let mut config = EwdsClientConfig::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000);
    // Independent cursor: observing FOS notifications must not consume submissions
    // from the community client's cursor, or responses used by the E2E queries.
    config.consumer_client_id = format!("gsye2estatus{}", Uuid::new_v4().simple());
    let client = EwdsClient::new(config);
    let (sender, receiver) = mpsc::unbounded_channel();
    let worker = tokio::spawn(async move {
        client
            .run_event_worker(EwdsEventType::OrderStatus, |event| {
                let sender = sender.clone();
                async move {
                    for order in serde_json::from_value::<Vec<EwdsOrderDto>>(event.data)? {
                        sender.send(order)?;
                    }
                    Ok(())
                }
            })
            .await;
    });
    world.order_status_observer = Some(OrderStatusObserver {
        worker,
        receiver,
        received: vec![],
    });
}

#[then(expr = "FOS receives {string} notifications for both traded orders")]
async fn verify_order_notifications(world: &mut MyWorld, status: String) {
    let trade = world
        .last_trade
        .as_ref()
        .expect("Missing settled trade")
        .clone();
    for id in [&trade.bid_hash, &trade.offer_hash] {
        let notification = world
            .order_status_observer
            .as_mut()
            .expect("FOS is not listening")
            .wait_for(id, &status)
            .await;
        assert_eq!(notification.market_id, trade.market_id);
        assert!(notification.reject_reason.is_none());
        if status == "executed" {
            let timestamp =
                super::trade_steps::delivery_verification_timestamp(world, &trade).await;
            assert_eq!(notification.updated_at, Some(epoch_to_rfc3339(timestamp)));
        }
    }
}

#[then("FOS receives a rejection for an order in a closed market without storing it")]
async fn verify_rejection_notification(world: &mut MyWorld) {
    let mut order = order_dto(world, "alice", true, 1.0, 20.0);
    // A fresh, never-opened market guarantees a contract rejection, not a timing race.
    order.market_id = bytes16_to_hex(*Uuid::new_v4().as_bytes());
    EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000)
        .publish_event(&event(EwdsEventType::Order, vec![order.clone()]))
        .await
        .unwrap();
    let rejected = world
        .order_status_observer
        .as_mut()
        .expect("FOS is not listening")
        .wait_for(&order.order_id, "rejected")
        .await;
    assert!(!rejected
        .reject_reason
        .as_deref()
        .unwrap_or_default()
        .is_empty());
    let mut expected = order.clone();
    expected.order_status = "rejected".into();
    expected.updated_at = rejected.updated_at.clone();
    expected.reject_reason = rejected.reject_reason.clone();
    assert_eq!(
        serde_json::to_value(rejected).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let registry = OrderRegistryContract::new(world.order_registry_address, world.provider.clone());
    assert_eq!(
        registry
            .get_status(parse_uuid_or_hex_bytes16(&order.order_id).unwrap())
            .call()
            .await
            .unwrap(),
        0
    );
    assert!(
        query_orders_for_market(world, &order.market_id)
            .await
            .is_empty(),
        "Rejected orders must not be stored"
    );
}

fn event<T>(event_type: EwdsEventType, data: T) -> EwdsEventEnvelope<T> {
    EwdsEventEnvelope {
        event_id: Uuid::new_v4().to_string(),
        event_type,
        occurred_at: epoch_to_rfc3339(get_current_timestamp_in_secs()),
        data,
    }
}

#[when("a site, a facility and a measurement batch are published on EWDS")]
async fn publish_events(world: &mut MyWorld) {
    let suffix = Uuid::new_v4();
    let facility_id = format!("e2e-event-facility-{}", suffix);
    let site = SiteSchema {
        site_name: format!("E2E Event Site {}", suffix),
        site_description: "Site sent by the events scenario".to_string(),
        facilities: vec![facility_id.clone()],
    };
    let facility = FacilitySchema {
        facility_id: facility_id.clone(),
        facility_name: format!("E2E Event Facility {}", suffix),
        site_id: site.site_name.clone(),
        owner_id: "alice".to_string(),
    };
    let (last_timeslot, _) = get_last_and_next_timeslot();
    let measurements = [(last_timeslot - 900, 1.5), (last_timeslot, -0.5)]
        .into_iter()
        .map(|(time_slot, energy_kwh)| MeasurementSchema {
            facility_id: facility_id.clone(),
            community_uuid: world.community_id.clone(),
            time_slot,
            creation_time: time_slot + 60,
            energy_kwh,
        })
        .collect::<Vec<_>>();

    let client = EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000);
    client
        .publish_event(&event(EwdsEventType::Site, vec![&site]))
        .await
        .expect("Failed to publish the site event");
    client
        .publish_event(&event(EwdsEventType::Facility, vec![&facility]))
        .await
        .expect("Failed to publish the facility event");
    let batch = measurements
        .iter()
        .cloned()
        .map(EwdsMeasurementDto::from)
        .collect::<Vec<_>>();
    client
        .publish_event(&event(EwdsEventType::Measurements, batch))
        .await
        .expect("Failed to publish the measurements event");

    world.submitted_site = Some(site);
    world.submitted_facility = Some(facility);
    world.submitted_measurements = measurements;
}

fn order_dto(
    world: &MyWorld,
    user_name: &str,
    is_bid: bool,
    quantity: f64,
    price_limit: f64,
) -> EwdsOrderDto {
    EwdsOrderDto {
        order_id: Uuid::new_v4().to_string(),
        market_id: market_id_as_hex(world),
        order_type: if is_bid { "bid" } else { "offer" }.to_string(),
        order_status: "submitted".to_string(),
        time_slot: epoch_to_rfc3339(world.target_delivery_time),
        quantity,
        price_limit,
        energy_source_preference: None,
        energy_type: None,
        created_by: user_name.to_string(),
        creation_time: epoch_to_rfc3339(get_current_timestamp_in_secs()),
        updated_at: None,
        reject_reason: None,
        preferred_trading_partner: None,
        preferred_energy_rate: None,
    }
}

/// Publishes one `order.event` with a bid and an offer, which the community client
/// places on-chain. Waits until both are indexed, then mines to the next matching block.
#[when(expr = "a bid by {string} and an offer by {string} are published on EWDS")]
async fn publish_order_event(world: &mut MyWorld, buyer_name: String, seller_name: String) {
    let bid = order_dto(world, buyer_name.as_str(), true, 2.0, 20.0);
    let offer = order_dto(world, seller_name.as_str(), false, 2.0, 5.0);
    // Both orders take one block each, and must land in the same matching interval.
    align_to_matching_window(world, 2).await;

    EwdsClient::from_env("EWDS_E2E_CLIENT_ID", "gsye2e", 60_000)
        .publish_event(&event(
            EwdsEventType::Order,
            vec![bid.clone(), offer.clone()],
        ))
        .await
        .expect("Failed to publish the order event");

    // The community client uses the UUID as the on-chain order ID, so the off-chain storage
    // indexes it as 0x plus the UUID's hex digits.
    for order in [&bid, &offer] {
        let onchain_order_id = bytes16_to_hex(
            parse_uuid_or_hex_bytes16(order.order_id.as_str()).expect("order IDs are UUIDs"),
        );
        wait_for_order_in_offchain_storage(world, onchain_order_id.as_str()).await;
    }

    mine_until_matching_block(
        world,
        primitives::matching::matching_block_interval() as usize + 1,
    )
    .await;
}

#[then("the off-chain storage stores the site and the facility")]
async fn verify_site_and_facility(world: &mut MyWorld) {
    let site = world.submitted_site.clone().expect("No site was published");
    let facility = world
        .submitted_facility
        .clone()
        .expect("No facility was published");

    for _ in 0..EVENT_POLL_ATTEMPTS {
        let sites = world
            .http_client
            .get(format!("{}/sites", world.offchain_storage_url))
            .send()
            .await
            .expect("Failed to fetch sites")
            .json::<Vec<SiteSchema>>()
            .await
            .expect("Failed to parse sites");
        let facilities = world
            .http_client
            .get(format!("{}/facilities", world.offchain_storage_url))
            .send()
            .await
            .expect("Failed to fetch facilities")
            .json::<Vec<FacilitySchema>>()
            .await
            .expect("Failed to parse facilities");
        if sites.contains(&site) && facilities.contains(&facility) {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: site {} and facility {} published on EWDS were not stored",
        site.site_name, facility.facility_id
    );
}

#[then("the off-chain storage stores the measurement batch")]
async fn verify_measurement_batch(world: &mut MyWorld) {
    let facility_id = world
        .submitted_facility
        .as_ref()
        .expect("No facility was published")
        .facility_id
        .clone();
    // The measurements adapter doesn't return the creation time, so it isn't compared.
    let expected = world
        .submitted_measurements
        .iter()
        .map(|measurement| {
            (
                measurement.community_uuid.clone(),
                measurement.time_slot,
                measurement.energy_kwh,
            )
        })
        .collect::<Vec<_>>();

    for _ in 0..EVENT_POLL_ATTEMPTS {
        let mut stored = world
            .http_client
            .get(format!(
                "{}/measurements?facility_id={}",
                world.offchain_storage_url, facility_id
            ))
            .send()
            .await
            .expect("Failed to fetch measurements")
            .json::<Vec<MeasurementSchema>>()
            .await
            .expect("Failed to parse measurements")
            .into_iter()
            .map(|measurement| {
                (
                    measurement.community_uuid,
                    measurement.time_slot,
                    measurement.energy_kwh,
                )
            })
            .collect::<Vec<_>>();
        stored.sort_by_key(|(_, time_slot, _)| *time_slot);
        if stored == expected {
            return;
        }
        sleep(Duration::from_secs(2)).await;
    }

    panic!(
        "Timeout: the measurement batch for facility {} published on EWDS was not stored",
        facility_id
    );
}
