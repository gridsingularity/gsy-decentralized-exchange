use crate::world::MyWorld;
use cucumber::{then, when};
use gsy_community_client::time_utils::{get_current_timestamp_in_secs, get_last_and_next_timeslot};
use primitives::db_api_schema::grid_topology::{FacilitySchema, SiteSchema};
use primitives::db_api_schema::profiles::MeasurementSchema;
use primitives::ewds::dto::{EwdsEventEnvelope, EwdsMeasurementDto};
use primitives::ewds::{EwdsClient, EwdsEventType};
use primitives::utils::epoch_to_rfc3339;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

const EVENT_POLL_ATTEMPTS: usize = 60;

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
        .publish_event(&event(EwdsEventType::SiteSubmitted, vec![&site]))
        .await
        .expect("Failed to publish the site event");
    client
        .publish_event(&event(EwdsEventType::FacilitySubmitted, vec![&facility]))
        .await
        .expect("Failed to publish the facility event");
    let batch = measurements
        .iter()
        .cloned()
        .map(EwdsMeasurementDto::from)
        .collect::<Vec<_>>();
    client
        .publish_event(&event(EwdsEventType::MeasurementsSubmitted, batch))
        .await
        .expect("Failed to publish the measurements event");

    world.submitted_site = Some(site);
    world.submitted_facility = Some(facility);
    world.submitted_measurements = measurements;
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
