//! `SPOT_MARKET_COMMUNITIES`: only the listed sites get a spot market per delivery slot.

use gsy_community_client::constants::DEFAULT_SPOT_MARKET_COMMUNITIES;
use gsy_community_client::inter_community::parse_community_list;
use gsy_community_client::offchain_storage_connector::adapter::AreaMarketInfoAdapter;
use gsy_community_client::topology::{
    ExternalCommunityTopology, LECCommunityAssetsResults, LECCommunityMembersResults, RawOntology,
    TopologyManager, retain_spot_market_communities,
};
use reqwest::Client;

// Live ontology responses, captured on 2026-09-26 (shared with `sites.rs`).
const LECS_BUILDINGS: &str = include_str!("fixtures/lecs_buildings.json");
const ASSETS_PILOT1: &str = include_str!("fixtures/assets_pilot1.json");
const ASSETS_PILOT2: &str = include_str!("fixtures/assets_pilot2.json");
const ASSETS_PILOT3: &str = include_str!("fixtures/assets_pilot3.json");

#[cfg(test)]
mod tests {
    use super::*;

    fn community(name: &str) -> ExternalCommunityTopology {
        ExternalCommunityTopology {
            community_name: name.to_string(),
            areas: vec![],
        }
    }

    fn names(communities: &[ExternalCommunityTopology]) -> Vec<&str> {
        communities
            .iter()
            .map(|community| community.community_name.as_str())
            .collect()
    }

    fn live_sites() -> Vec<ExternalCommunityTopology> {
        let assets = |lec: &str, json: &str| -> (String, LECCommunityAssetsResults) {
            (
                lec.to_string(),
                serde_json::from_str(json).expect("assets fixture parses"),
            )
        };
        let buildings: LECCommunityMembersResults =
            serde_json::from_str(LECS_BUILDINGS).expect("lecs_buildings.json parses");
        let raw = RawOntology {
            buildings,
            assets: vec![
                assets("Pilot1", ASSETS_PILOT1),
                assets("Pilot2", ASSETS_PILOT2),
                assets("Pilot3", ASSETS_PILOT3),
            ],
        };
        TopologyManager::new(&Client::new(), &AreaMarketInfoAdapter::new(None))
            .communities_by_site(&raw)
    }

    #[test]
    fn empty_list_keeps_every_community() {
        let kept = retain_spot_market_communities(vec![community("A"), community("B")], &[]);
        assert_eq!(names(&kept), vec!["A", "B"]);
    }

    #[test]
    fn only_listed_communities_are_kept_in_their_original_order() {
        let allowed = parse_community_list("C, A");
        let kept = retain_spot_market_communities(
            vec![community("A"), community("B"), community("C")],
            &allowed,
        );
        assert_eq!(names(&kept), vec!["A", "C"]);
    }

    #[test]
    fn a_listed_name_without_a_site_is_ignored() {
        let allowed = parse_community_list("A,NoSuchSite");
        let kept = retain_spot_market_communities(vec![community("A"), community("B")], &allowed);
        assert_eq!(names(&kept), vec!["A"]);
    }

    #[test]
    fn default_keeps_the_three_forecast_sites_of_the_live_ontology() {
        let sites = live_sites();
        assert_eq!(sites.len(), 9, "the live ontology has nine sites");

        let kept = retain_spot_market_communities(
            sites,
            &parse_community_list(DEFAULT_SPOT_MARKET_COMMUNITIES),
        );
        assert_eq!(
            names(&kept),
            vec![
                "ArenaInnovationCommunity",
                "GaramèDistrict",
                "LugaggiaInnovationCommunity"
            ]
        );
        assert!(kept.iter().all(|community| !community.areas.is_empty()));
    }
}
