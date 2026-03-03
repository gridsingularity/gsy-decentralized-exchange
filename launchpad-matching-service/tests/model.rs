use launchpad_matching_service::api::model::MatchStore;
use launchpad_matching_service::api::types::DbBidOfferMatch;
use gsy_offchain_primitives::db_api_schema::orders::{DbBid, DbOffer, DbOrderComponent};
use async_trait::async_trait;
use mockall::mock;

mock! {
    pub MatchStore {}
    #[async_trait]
    impl MatchStore for MatchStore {
        async fn insert_matches(&self, matches: Vec<DbBidOfferMatch>) -> mongodb::error::Result<()>;
    }
}

#[tokio::test]
async fn test_insert_matches_mocked() {
    let mut mock = MockMatchStore::new();
    
    let market_id = "test_market".to_string();
    let bid = DbBid {
        buyer: "buyer1".to_string(),
        nonce: 1,
        bid_component: DbOrderComponent {
            area_uuid: "area1".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 15.0,
        },
    };

    let offer = DbOffer {
        seller: "seller1".to_string(),
        nonce: 1,
        offer_component: DbOrderComponent {
            area_uuid: "area2".to_string(),
            market_id: market_id.clone(),
            time_slot: 100,
            creation_time: 100,
            energy: 10.0,
            energy_rate: 10.0,
        },
    };

    let matches = vec![DbBidOfferMatch {
        market_id,
        time_slot: 100,
        bid,
        offer,
        residual_bid: None,
        residual_offer: None,
        selected_energy: 10.0,
        energy_rate: 15.0,
    }];

    let matches_clone = matches.clone();

    mock.expect_insert_matches()
        .with(mockall::predicate::eq(matches_clone))
        .times(1)
        .returning(|_| Ok(()));

    let result = mock.insert_matches(matches).await;
    assert!(result.is_ok());
}
