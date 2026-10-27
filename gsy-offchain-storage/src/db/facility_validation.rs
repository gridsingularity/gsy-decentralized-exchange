use super::DatabaseWrapper;
use anyhow::{Context, Result};
use futures::TryStreamExt;
use mongodb::bson::doc;
use primitives::db_api_schema::grid_topology::{
    EnergyCommunitySchema, FacilityMarketValidationReason, FacilityMarketValidationRequest,
    FacilityMarketValidationResult, FacilitySchema, SiteSchema,
};
use primitives::db_api_schema::market::MarketSchema;
use primitives::utils::{bytes16_to_hex, parse_uuid_or_hex_bytes16};
use std::collections::HashMap;

impl DatabaseWrapper {
    /// Checks the complete facility -> site -> market community chain without writing data.
    /// Results preserve request order and duplicates. Database errors fail the whole lookup,
    /// rather than being reported as confirmed eligibility failures for individual pairs.
    pub async fn validate_facility_markets(
        &self,
        requests: &[FacilityMarketValidationRequest],
    ) -> Result<Vec<FacilityMarketValidationResult>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let market_ids: Vec<String> = requests
            .iter()
            .map(|pair| {
                parse_uuid_or_hex_bytes16(&pair.market_id)
                    .map(bytes16_to_hex)
                    .unwrap_or_else(|| pair.market_id.clone())
            })
            .collect();
        let facility_ids: Vec<&str> = requests
            .iter()
            .map(|pair| pair.facility_id.as_str())
            .collect();
        let markets: Vec<MarketSchema> = self
            .markets()
            .find(doc! {"market_id": {"$in": &market_ids}})
            .await?
            .try_collect()
            .await
            .context("Failed to read markets for facility validation")?;
        let facilities: Vec<FacilitySchema> = self
            .facilities()
            .find(doc! {"facility_id": {"$in": facility_ids}})
            .await?
            .try_collect()
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
        let communities: Vec<EnergyCommunitySchema> = self
            .communities()
            .find(doc! {"community_id": {"$in": community_ids}})
            .await?
            .try_collect()
            .await
            .context("Failed to read communities for facility validation")?;
        let sites: Vec<SiteSchema> = self
            .sites()
            .find(doc! {"site_name": {"$in": site_ids}})
            .await?
            .try_collect()
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
        let sites: HashMap<_, _> = sites.iter().map(|site| (&site.site_name, site)).collect();

        let validate_pair = |pair: &FacilityMarketValidationRequest, market_id: &String| {
            use FacilityMarketValidationReason::*;
            let market = markets.get(market_id).ok_or(UnknownMarket)?;
            let facility = facilities.get(&pair.facility_id).ok_or(UnknownFacility)?;
            let community = communities
                .get(&market.community_id)
                .ok_or(UnknownCommunity)?;
            // SiteSchema is keyed by site_name; FacilitySchema.site_id refers to that name.
            let site = sites.get(&facility.site_id).ok_or(UnknownSite)?;
            if !site.facilities.contains(&facility.facility_id) {
                return Err(FacilityNotListedInSite);
            }
            if !community.sites.contains(&site.site_name) {
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
