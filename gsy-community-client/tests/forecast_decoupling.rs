use gsy_community_client::node_connector::orders::create_input_orders;
use gsy_community_client::node_connector::orders::gsy_node::runtime_types::gsy_primitives::orders::InputOrder;
use gsy_community_client::offchain_storage_connector::adapter::{
    build_new_market_topology, deterministic_area_hash, deterministic_area_uuid,
    deterministic_community_uuid,
};
use gsy_community_client::topology::{ExternalAreaTopology, ExternalCommunityTopology};
use gsy_offchain_primitives::db_api_schema::market::AssetType;
use gsy_offchain_primitives::db_api_schema::profiles::ForecastSchema;
use gsy_offchain_primitives::utils::h256_to_string;
use subxt_signer::sr25519::dev;

/// The publish side of the decoupled pipeline consumes forecasts read from storage and
/// reconstructs orders from them, with no forecaster involved. This exercises that
/// reconstruction directly: forecasts stamped with the same deterministic identity the
/// ingestion loop writes are fed through `build_new_market_topology` + `create_input_orders`,
/// asserting a positive-energy (consumption) forecast becomes a bid and a negative-energy
/// (PV production) forecast becomes an offer. The storage HTTP round-trip and upsert
/// semantics are covered by the gsy-offchain-storage crate's own tests.
#[tokio::test]
async fn orders_are_reconstructed_from_forecasts_without_any_forecaster_call() {
    let community_name = "TestCommunity";
    let topology = ExternalCommunityTopology {
        community_name: community_name.to_string(),
        areas: vec![
            ExternalAreaTopology {
                area_name: "DemandMeter".to_string(),
                area_type: AssetType::SMART_METER,
            },
            ExternalAreaTopology {
                area_name: "PvAsset".to_string(),
                area_type: AssetType::PV,
            },
        ],
    };

    let now = 1_800_000_000u64;
    let open_timeslot = now + 900;

    let market = build_new_market_topology(&topology, open_timeslot);
    assert_eq!(
        market.community_uuid,
        deterministic_community_uuid(community_name),
        "the market the publish side builds must share the ingestion side's community_uuid"
    );

    // Forecasts as they come back from storage: same deterministic identity the ingestion
    // loop stamped, so their area_hash lines up with the market areas.
    let forecasts = vec![
        ForecastSchema {
            area_uuid: deterministic_area_uuid(community_name, "DemandMeter"),
            area_hash: h256_to_string(deterministic_area_hash(community_name, "DemandMeter")),
            community_uuid: deterministic_community_uuid(community_name),
            time_slot: open_timeslot,
            creation_time: now,
            energy_kwh: 4.0, // positive: consumption -> bid.
            confidence: 0.9,
        },
        ForecastSchema {
            area_uuid: deterministic_area_uuid(community_name, "PvAsset"),
            area_hash: h256_to_string(deterministic_area_hash(community_name, "PvAsset")),
            community_uuid: deterministic_community_uuid(community_name),
            time_slot: open_timeslot,
            creation_time: now,
            energy_kwh: -2.5, // negative: production -> offer.
            confidence: 0.7,
        },
    ];

    let input_orders =
        create_input_orders(forecasts, market, 0.2, now, now + 900, &dev::alice());

    assert_eq!(input_orders.len(), 2);
    let bids = input_orders
        .iter()
        .filter(|order| matches!(order, InputOrder::Bid(_)))
        .count();
    let offers = input_orders
        .iter()
        .filter(|order| matches!(order, InputOrder::Offer(_)))
        .count();
    assert_eq!(bids, 1, "positive-energy demand forecast must reconstruct into a bid");
    assert_eq!(offers, 1, "negative-energy PV forecast must reconstruct into an offer");
}
