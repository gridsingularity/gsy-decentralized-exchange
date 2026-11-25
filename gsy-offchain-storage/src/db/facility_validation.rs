use super::DatabaseWrapper;
use anyhow::{Context, Result};
use primitives::db_api_schema::market::MarketSchema;
use primitives::ewds::dto::{
    FacilityMarketValidationReason, FacilityMarketValidationRequest, FacilityMarketValidationResult,
};
use primitives::utils::{validate_offchain_actor_id, normalize_uuid};
use std::collections::HashMap;

impl DatabaseWrapper {
    /// Checks the complete facility -> site -> market community chain without writing data,
    /// using the latest version of each facility, site and community; deleted ones are unknown.
    /// Results preserve request order and duplicates. Database errors fail the whole lookup,
    /// rather than being reported as confirmed eligibility failures for individual pairs.
    pub async fn validate_facility_markets(
        &self,
        requests: &[FacilityMarketValidationRequest],
    ) -> Result<Vec<FacilityMarketValidationResult>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let market_ids: Vec<Option<String>> = requests
            .iter()
            .map(|pair| normalize_uuid(&pair.market_id).ok())
            .collect();
        let facility_ids: Vec<String> = requests
            .iter()
            .filter_map(|pair| validate_offchain_actor_id(&pair.facility_id).ok())
            .collect();
        let markets: Vec<MarketSchema> = self
            .markets()
            .get_by_ids(market_ids.iter().flatten().cloned().collect())
            .await
            .context("Failed to read markets for facility validation")?;
        let facilities = self
            .facilities()
            .get_by_ids(&facility_ids.iter().map(String::as_str).collect::<Vec<_>>())
            .await
            .context("Failed to read facilities for market validation")?;

        let community_ids: Vec<&str> = markets
            .iter()
            .map(|market| market.community_id.as_str())
            .collect();
        let site_ids: Vec<&str> = facilities
            .iter()
            .map(|facility| facility.site_id.as_str())
            .collect();
        let communities = self
            .communities()
            .get_by_ids(&community_ids)
            .await
            .context("Failed to read communities for facility validation")?;
        let sites = self
            .sites()
            .get_by_ids(&site_ids)
            .await
            .context("Failed to read sites for facility validation")?;

        let markets: HashMap<_, _> = markets
            .iter()
            .map(|market| (&market.market_id, market))
            .collect();
        let facilities: HashMap<_, _> = facilities
            .iter()
            .map(|facility| (&facility.facility_id, facility))
            .collect();
        let communities: HashMap<_, _> = communities
            .iter()
            .map(|community| (&community.community_id, community))
            .collect();
        let sites: HashMap<_, _> = sites.iter().map(|site| (&site.site_id, site)).collect();

        let validate_pair = |pair: &FacilityMarketValidationRequest, market_id: &Option<String>| {
            use FacilityMarketValidationReason::*;
            let market = markets
                .get(market_id.as_ref().ok_or(UnknownMarket)?)
                .ok_or(UnknownMarket)?;
            let facility_id =
                validate_offchain_actor_id(&pair.facility_id).map_err(|_| UnknownFacility)?;
            let facility = facilities.get(&facility_id).ok_or(UnknownFacility)?;
            let community = communities
                .get(&market.community_id)
                .ok_or(UnknownCommunity)?;
            let site = sites.get(&facility.site_id).ok_or(UnknownSite)?;
            if !site.facilities.contains(&facility.facility_id) {
                return Err(FacilityNotListedInSite);
            }
            if !community.sites.contains(&site.site_id) {
                return Err(SiteNotInCommunity);
            }
            Ok(())
        };

        Ok(requests
            .iter()
            .zip(&market_ids)
            .map(|(pair, market_id)| {
                let reason = validate_pair(pair, market_id).err();
                FacilityMarketValidationResult {
                    facility_id: pair.facility_id.clone(),
                    market_id: pair.market_id.clone(),
                    eligible: reason.is_none(),
                    reason,
                }
            })
            .collect())
    }
}
