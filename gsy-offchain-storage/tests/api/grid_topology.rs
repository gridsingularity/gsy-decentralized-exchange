use crate::helpers::{init_app, stop_app};
use gsy_offchain_primitives::db_api_schema::grid_topology::{
    AssetSchema, AssetType, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use gsy_offchain_primitives::db_api_schema::tariff::TariffSchema;

fn base_asset(uuid: &str, asset_type: AssetType, asset_name: &str) -> AssetSchema {
    AssetSchema {
        asset_type,
        uuid: uuid.to_string(),
        asset_name: asset_name.to_string(),
        facility_name: "Building 1".to_string(),
        creation_time: 1546300800,
        installed_power: 5.0,
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
            ..base_asset("energy_asset_uuid", AssetType::EnergyAsset, "Home 1 Consumption")
        },
        AssetSchema {
            asset_subtype: Some("Li-ion".to_string()),
            technology_type: Some("SMA".to_string()),
            installed_power: 5.0,
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

    let resp = client.get(&format!("{}/assets", &address)).send().await.unwrap();
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
async fn post_community_site_facility() {
    let app = init_app().await;
    let address = app.address.clone();
    let client = reqwest::Client::new();

    let community = EnergyCommunitySchema {
        community_name: "Aran Islands REC".to_string(),
        sites: vec!["Aran Islands Site 1".to_string()],
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/communities", &address))
            .json(&community)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    );

    let site = SiteSchema {
        site_name: "Aran Islands Site 1".to_string(),
        site_description: "Group of 15 residential buildings.".to_string(),
        facilities: vec!["AIS1-House-1".to_string()],
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/sites", &address))
            .json(&site)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    );

    let facility = FacilitySchema {
        facility_name: "AIS1-House-1".to_string(),
        address: "Random str. 15, 12345 (anonymized)".to_string(),
        latitude: 53.12,
        longitude: -9.65,
        category: "residential".to_string(),
        number_of_occupants: 4,
    };
    assert_eq!(
        200,
        client
            .post(&format!("{}/facilities", &address))
            .json(&facility)
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
