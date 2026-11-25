//! `valid_from` and `valid_until` of communities, sites and facilities: RFC 3339 strings, stored
//! in UTC with whole seconds, as `epoch_to_rfc3339` writes them.

use primitives::db_api_schema::grid_topology::{EnergyCommunitySchema, FacilitySchema, SiteSchema};
use primitives::ewds::dto::{EwdsCommunityDto, EwdsFacilityDto, EwdsSiteDto};
use primitives::utils::{epoch_to_rfc3339, EPOCH_RFC3339};
use serde_json::json;

fn site_dto(valid_until: Option<&str>) -> EwdsSiteDto {
    EwdsSiteDto {
        site_id: "site-1".to_string(),
        site_name: "Site 1".to_string(),
        site_description: "Main building".to_string(),
        facilities: vec!["facility-1".to_string()],
        valid_from: None,
        valid_until: valid_until.map(str::to_string),
    }
}

fn stored_valid_until(valid_until: &str) -> String {
    SiteSchema::try_from(site_dto(Some(valid_until)))
        .unwrap()
        .valid_until
        .unwrap()
}

#[test]
fn valid_until_is_read_with_any_offset_and_stored_in_utc() {
    let site = SiteSchema::try_from(site_dto(Some("2026-12-01T01:00:00+01:00"))).unwrap();
    assert_eq!(
        site.valid_until.as_deref(),
        Some("2026-12-01T00:00:00+00:00")
    );
    // Without valid_from, the entry reads as the oldest possible.
    assert_eq!(site.valid_from, EPOCH_RFC3339);

    let written = serde_json::to_value(EwdsSiteDto::from(site)).unwrap();
    assert_eq!(written["valid_until"], json!("2026-12-01T00:00:00+00:00"));
    assert_eq!(written["valid_from"], json!(EPOCH_RFC3339));
}

#[test]
fn stored_times_sort_as_strings_in_time_order() {
    // As sent, the earlier time sorts last.
    let earlier = "2026-12-01T00:30:00+01:00";
    let later = "2026-11-30T23:45:00Z";
    assert!(earlier > later);
    assert!(stored_valid_until(earlier) < stored_valid_until(later));
}

#[test]
fn fractions_of_a_second_are_dropped() {
    assert_eq!(
        stored_valid_until("2026-12-01T00:00:00.900Z"),
        "2026-12-01T00:00:00+00:00"
    );
}

#[test]
fn a_missing_valid_until_is_left_out_and_null_reads_as_none() {
    let facility = FacilitySchema {
        facility_id: "facility-1".to_string(),
        facility_name: "Facility 1".to_string(),
        site_id: "site-1".to_string(),
        owner_id: "owner-1".to_string(),
        valid_from: "2026-12-01T00:00:00+00:00".to_string(),
        valid_until: None,
    };
    let written = serde_json::to_value(EwdsFacilityDto::from(facility.clone())).unwrap();
    assert!(written.get("valid_until").is_none(), "{written}");
    assert_eq!(written["valid_from"], json!("2026-12-01T00:00:00+00:00"));

    let mut with_null = written;
    with_null["valid_until"] = json!(null);
    let dto: EwdsFacilityDto = serde_json::from_value(with_null).unwrap();
    assert_eq!(FacilitySchema::try_from(dto).unwrap(), facility);
}

#[test]
fn invalid_times_are_rejected() {
    for valid_until in ["tomorrow", "2026-12-01", "1764547200"] {
        assert!(
            SiteSchema::try_from(site_dto(Some(valid_until))).is_err(),
            "{valid_until}"
        );
    }
    let mut dto = site_dto(None);
    dto.valid_from = Some("not a time".to_string());
    assert!(SiteSchema::try_from(dto).is_err());

    // A number is not an RFC 3339 string.
    let mut data = json!(site_dto(None));
    data["valid_until"] = json!(1_796_083_200);
    assert!(serde_json::from_value::<EwdsSiteDto>(data).is_err());
}

#[test]
fn the_database_schemas_store_strings_and_require_valid_from() {
    let community = EnergyCommunitySchema {
        community_id: "community-1".to_string(),
        community_name: "Community 1".to_string(),
        sites: vec![],
        valid_from: "2026-12-01T00:00:00+00:00".to_string(),
        valid_until: Some("2026-12-02T00:00:00+00:00".to_string()),
    };
    let written = serde_json::to_value(&community).unwrap();
    assert_eq!(written["valid_from"], json!("2026-12-01T00:00:00+00:00"));
    assert_eq!(written["valid_until"], json!("2026-12-02T00:00:00+00:00"));

    let mut without_valid_from = written;
    without_valid_from
        .as_object_mut()
        .unwrap()
        .remove("valid_from");
    assert!(serde_json::from_value::<EnergyCommunitySchema>(without_valid_from).is_err());
}

#[test]
fn the_oldest_valid_from_is_the_unix_epoch() {
    assert_eq!(epoch_to_rfc3339(0), EPOCH_RFC3339);
}

#[test]
fn the_community_dto_carries_valid_from_and_valid_until_in_camel_case() {
    let dto: EwdsCommunityDto = serde_json::from_value(json!({
        "communityId": "community-1",
        "communityName": "Community 1",
        "sites": ["site-1"],
        "validFrom": "2026-11-30T00:00:00Z",
        "validUntil": "2026-12-01T01:00:00+01:00",
    }))
    .unwrap();
    let community = EnergyCommunitySchema::try_from(dto).unwrap();
    assert_eq!(community.valid_from, "2026-11-30T00:00:00+00:00");
    assert_eq!(
        community.valid_until.as_deref(),
        Some("2026-12-01T00:00:00+00:00")
    );

    let written = serde_json::to_value(EwdsCommunityDto::from(community)).unwrap();
    assert_eq!(written["validFrom"], json!("2026-11-30T00:00:00+00:00"));
    assert_eq!(written["validUntil"], json!("2026-12-01T00:00:00+00:00"));
}
