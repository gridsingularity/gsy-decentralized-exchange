use crate::helpers::{init_app, stop_app, stored};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use primitives::db_api_schema::tariff::TariffSchema;
use primitives::ewds::dto::{EwdsCommunityDto, EwdsFacilityDto, EwdsSiteDto};
use primitives::utils::{epoch_to_rfc3339, now_epoch_secs, EPOCH_RFC3339};

fn base_asset(uuid: &str, asset_type: AssetType, asset_name: &str) -> AssetSchema {
    AssetSchema {
        asset_type,
        uuid: uuid.to_string(),
        asset_name: asset_name.to_string(),
        facility_name: "Building 1".to_string(),
        creation_time: 1546300800,
        installed_power: Some(5.0),
        asset_subtype: None,
        technology_type: None,
        phase_connection: None,
        energy_capacity: None,
        maximum_soc: None,
        minimum_soc: None,
        roundtrip_efficiency: None,
        target_service: None,
        grid_connection_type: None,
        max_rated_current: None,
        has_smart_meter: None,
        tariff_name: None,
    }
}

#[tokio::test]
async fn post_and_get_assets() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let assets = vec![
        AssetSchema {
            phase_connection: Some("single-phase".to_string()),
            ..base_asset(
                "energy_asset_uuid",
                AssetType::EnergyAsset,
                "Home 1 Consumption",
            )
        },
        AssetSchema {
            asset_subtype: Some("Li-ion".to_string()),
            technology_type: Some("SMA".to_string()),
            installed_power: Some(5.0),
            energy_capacity: Some(5.0),
            maximum_soc: Some(95.0),
            minimum_soc: Some(5.0),
            roundtrip_efficiency: Some(95.0),
            ..base_asset("battery_uuid", AssetType::Battery, "BAT-IE-007")
        },
    ];

    let resp = client
        .post(&format!("{}/assets", &address))
        .json(&assets)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/assets", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<AssetSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 2);

    let resp = client
        .get(&format!("{}/assets?uuid=battery_uuid", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let battery: AssetSchema = resp.json().await.unwrap();
    assert_eq!(battery.asset_type, AssetType::Battery);
    assert_eq!(battery.maximum_soc, Some(95.0));
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_pilot_site() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let pilot = PilotSiteSchema {
        pilot_name: "Aran Islands".to_string(),
        pilot_description: "Off-grid island demonstration.".to_string(),
        start_date: "2024-01-01".to_string(),
        end_date: "2027-12-31".to_string(),
        latitude: 53.12,
        longitude: -9.65,
        communities: vec!["Aran Islands REC".to_string()],
    };

    let resp = client
        .post(&format!("{}/pilot-sites", &address))
        .json(&pilot)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/pilot-sites", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<PilotSiteSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].pilot_name, "Aran Islands");
    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_communities() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let communities = vec![
        EnergyCommunitySchema {
            community_id: "11111111-1111-4111-8111-111111111111".to_string(),
            community_name: "Aran Islands REC".to_string(),
            sites: vec!["Aran Islands Site 1".to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
        EnergyCommunitySchema {
            community_id: "22222222-2222-4222-8222-222222222222".to_string(),
            community_name: "Lugaggia Innovation Community".to_string(),
            sites: vec!["Lugaggia Site 1".to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
    ];

    for community in &communities {
        let response = client
            .post(&format!("{}/communities", &address))
            .json(&EwdsCommunityDto::from(community.clone()))
            .send()
            .await
            .unwrap();
        assert_eq!(200, response.status().as_u16());
    }

    let response = client
        .get(&format!("{}/communities", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, response.status().as_u16());

    let returned: Vec<EwdsCommunityDto> = response.json().await.unwrap();
    let mut returned_ids = returned
        .into_iter()
        .map(|community| community.community_id)
        .collect::<Vec<_>>();
    returned_ids.sort();

    assert_eq!(
        returned_ids,
        vec![
            "11111111-1111-4111-8111-111111111111".to_string(),
            "22222222-2222-4222-8222-222222222222".to_string(),
        ]
    );
    stop_app(app).await;
}

#[tokio::test]
async fn upsert_community_is_idempotent() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();
    let community_id = "11111111-1111-4111-8111-111111111111";

    for community in [
        EnergyCommunitySchema {
            community_id: community_id.to_string(),
            community_name: "Original name".to_string(),
            sites: vec!["Original site".to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
        EnergyCommunitySchema {
            community_id: community_id.to_string(),
            community_name: "Updated name".to_string(),
            sites: vec!["Updated site".to_string()],
            valid_from: EPOCH_RFC3339.to_string(),
            valid_until: None,
        },
    ] {
        let response = client
            .post(&format!("{}/communities", &address))
            .json(&EwdsCommunityDto::from(community.clone()))
            .send()
            .await
            .unwrap();
        assert_eq!(200, response.status().as_u16());
    }

    let communities = stored(app.db_wrapper.communities()).await;
    assert_eq!(communities.len(), 1);
    assert_eq!(communities[0].community_id, community_id);
    assert_eq!(communities[0].community_name, "Updated name");
    assert_eq!(communities[0].sites, vec!["Updated site"]);

    stop_app(app).await;
}

#[tokio::test]
async fn post_community_site_facility() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let community = EnergyCommunitySchema {
        community_id: "11111111-1111-4111-8111-111111111111".to_string(),
        community_name: "Aran Islands REC".to_string(),
        sites: vec!["Aran Islands Site 1".to_string()],
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/communities", &address))
            .json(&EwdsCommunityDto::from(community.clone()))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    );

    let site = SiteSchema {
        site_id: "site 1".to_string(),
        site_name: "Aran Islands Site 1".to_string(),
        site_description: "Group of 15 residential buildings.".to_string(),
        facilities: vec!["AIS1-House-1".to_string()],
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/sites", &address))
            .json(&EwdsSiteDto::from(site.clone()))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    );

    let facility = FacilitySchema {
        facility_id: "AIS1-House-1".to_string(),
        facility_name: "AIS1-House-1".to_string(),
        site_id: "site 1".to_string(),
        owner_id: "owner 1".to_string(),
        valid_from: EPOCH_RFC3339.to_string(),
        valid_until: None,
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/facilities", &address))
            .json(&EwdsFacilityDto::from(facility.clone()))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    );

    stop_app(app).await;
}

#[tokio::test]
async fn post_and_get_tariff() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let tariff = TariffSchema {
        tariff_name: "ESB-Residential-TOU".to_string(),
        tariff_structure: "TOU".to_string(),
        energy_price: 0.28,
        grid_fee: 0.04,
        taxes: 0.03,
        incentives: 0.00,
        currency: "EUR".to_string(),
        tariff_start: "2026-03-27T19:00:00Z".to_string(),
        tariff_end: "2027-03-27T19:00:00Z".to_string(),
    };

    let resp = client
        .post(&format!("{}/tariffs", &address))
        .json(&tariff)
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());

    let resp = client
        .get(&format!("{}/tariffs", &address))
        .send()
        .await
        .unwrap();
    assert_eq!(200, resp.status().as_u16());
    let returned: Vec<TariffSchema> = resp.json().await.unwrap();
    assert_eq!(returned.len(), 1);
    assert_eq!(returned[0].tariff_name, "ESB-Residential-TOU");
    stop_app(app).await;
}

#[tokio::test]
async fn validity_fields_are_stored_as_utc_strings_and_startup_fills_a_missing_valid_from() {
    use gsy_offchain_storage::db::grid_topology_service::init_facilities;
    use mongodb::bson::{doc, Document};

    let app = init_app().await;
    let facility = FacilitySchema::try_from(EwdsFacilityDto {
        facility_id: "facility-1".to_string(),
        facility_name: "Facility 1".to_string(),
        site_id: "site-1".to_string(),
        owner_id: "owner-1".to_string(),
        valid_from: None,
        valid_until: Some("2026-12-02T01:00:00.5+01:00".to_string()),
    })
    .unwrap();
    let before = epoch_to_rfc3339(now_epoch_secs());
    app.db_wrapper
        .facilities()
        .store(vec![facility])
        .await
        .unwrap();
    let after = epoch_to_rfc3339(now_epoch_secs());
    let raw = app.db_wrapper.collection::<Document>("facilities");
    let stored = raw
        .find_one(doc! {"facility_id": "facility-1"})
        .await
        .unwrap()
        .unwrap();
    // The receive time.
    let valid_from = stored.get_str("valid_from").unwrap();
    assert!(
        before.as_str() <= valid_from && valid_from <= after.as_str(),
        "{valid_from}"
    );
    assert_eq!(
        stored.get_str("valid_until").unwrap(),
        "2026-12-02T00:00:00+00:00"
    );

    // A document from before versioning.
    raw.insert_one(doc! {
        "facility_id": "facility-0",
        "facility_name": "Facility 0",
        "site_id": "site-1",
        "owner_id": "owner-1",
    })
    .await
    .unwrap();
    init_facilities(&app.db_wrapper).await.unwrap();
    let old = app
        .db_wrapper
        .collection::<FacilitySchema>("facilities")
        .find_one(doc! {"facility_id": "facility-0"})
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (old.valid_from.as_str(), old.valid_until.clone()),
        (EPOCH_RFC3339, None)
    );

    // It is read like any entry, and a new version of it replaces it.
    let facility_0 = || async {
        app.db_wrapper
            .facilities()
            .get()
            .await
            .unwrap()
            .into_iter()
            .find(|facility| facility.facility_id == "facility-0")
    };
    assert_eq!(facility_0().await.unwrap().facility_name, "Facility 0");
    let renamed = FacilitySchema {
        facility_name: "Facility 0 renamed".to_string(),
        ..old
    };
    app.db_wrapper
        .facilities()
        .store(vec![renamed])
        .await
        .unwrap();
    assert_eq!(
        facility_0().await.unwrap().facility_name,
        "Facility 0 renamed"
    );

    stop_app(app).await;
}

#[tokio::test]
async fn startup_replaces_the_unique_indexes_with_one_on_id_and_valid_from() {
    use gsy_offchain_storage::db::grid_topology_service::{
        init_communities, init_facilities, init_sites,
    };
    use mongodb::bson::{doc, Document};
    use mongodb::options::IndexOptions;
    use mongodb::IndexModel;

    let app = init_app().await;
    let db = &app.db_wrapper;
    // A deployment from before, with the unique indexes of the old code.
    for (collection, fields) in [
        ("communities", &["community_id", "community_name"][..]),
        ("sites", &["site_name"][..]),
        ("facilities", &["facility_name", "facility_id"][..]),
    ] {
        let collection = db.collection::<Document>(collection);
        collection.drop_indexes().await.unwrap();
        for field in fields {
            collection
                .create_index(
                    IndexModel::builder()
                        .keys(doc! {*field: 1})
                        .options(IndexOptions::builder().unique(true).build())
                        .build(),
                )
                .await
                .unwrap();
        }
    }

    // Twice, as on every restart.
    for _ in 0..2 {
        init_communities(db).await.unwrap();
        init_sites(db).await.unwrap();
        init_facilities(db).await.unwrap();
    }

    for (collection, index) in [
        ("communities", "community_id_1_valid_from_-1"),
        ("sites", "site_id_1_valid_from_-1"),
        ("facilities", "facility_id_1_valid_from_-1"),
    ] {
        let mut names = db
            .collection::<Document>(collection)
            .list_index_names()
            .await
            .unwrap();
        names.sort();
        assert_eq!(names, vec!["_id_", index], "{collection}");
    }

    // An id and a name can now have several documents.
    let facilities = db.collection::<Document>("facilities");
    for _ in 0..2 {
        facilities
            .insert_one(doc! {"facility_id": "facility-1", "facility_name": "Facility 1"})
            .await
            .unwrap();
    }
    assert_eq!(
        facilities
            .count_documents(doc! {"facility_id": "facility-1"})
            .await
            .unwrap(),
        2
    );

    stop_app(app).await;
}
