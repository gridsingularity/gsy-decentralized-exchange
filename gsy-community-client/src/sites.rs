//! The site of an ontology asset. Every site (`siteName`) is its own community, and so its
//! own spot market. Both the market topology and the metering points resolve an asset's
//! site here, so an asset's market and its metering point always agree.
//!
//! A building member belongs to its building's site (`participantName` → `siteName` of
//! `get_lecs_buildings`); a site-level asset, whose `location` is a `siteName` of its LEC,
//! belongs to that site; any other asset has no site.

use crate::topology::{ExternalCommunityAsset, LECCommunityMembersResults};
use std::collections::{BTreeSet, HashMap};

/// The buildings and sites of every LEC, as read from the `get_lecs_buildings` response.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteIndex {
    /// Per LEC, the site of each of its buildings.
    building_sites: HashMap<String, HashMap<String, String>>,
    /// Per LEC, its sites.
    lec_sites: HashMap<String, BTreeSet<String>>,
}

impl SiteIndex {
    /// Index the buildings and sites of `buildings`. A building listed under two sites of
    /// its LEC belongs to the one that sorts first, so the result does not depend on the
    /// order of the rows.
    pub fn new(buildings: &LECCommunityMembersResults) -> Self {
        let mut index = SiteIndex::default();
        for row in &buildings.results.bindings {
            let lec = &row.lec_name.value;
            let site = &row.site_name.value;
            index
                .lec_sites
                .entry(lec.clone())
                .or_default()
                .insert(site.clone());
            let site_of_building = index
                .building_sites
                .entry(lec.clone())
                .or_default()
                .entry(row.participant_name.value.clone())
                .or_insert_with(|| site.clone());
            if site < site_of_building {
                *site_of_building = site.clone();
            }
        }
        index
    }

    /// The site of `building` if it is a building of `lec`.
    pub fn building_site(&self, lec: &str, building: &str) -> Option<&str> {
        self.building_sites
            .get(lec)?
            .get(building)
            .map(String::as_str)
    }

    /// Whether `site` is a site of `lec`.
    pub fn is_site(&self, lec: &str, site: &str) -> bool {
        self.lec_sites
            .get(lec)
            .is_some_and(|sites| sites.contains(site))
    }

    /// The site that a `location` fragment of an asset of `lec` belongs to: the building's
    /// site for a building of `lec`, the site itself for a site of `lec`, `None` otherwise.
    pub fn site_of_location(&self, lec: &str, location: &str) -> Option<&str> {
        if let Some(site) = self.building_site(lec, location) {
            return Some(site);
        }
        self.lec_sites.get(lec)?.get(location).map(String::as_str)
    }

    /// The site of an asset of `lec`, from its ontology `location`.
    pub fn site_of_asset(&self, lec: &str, asset: &ExternalCommunityAsset) -> Option<&str> {
        self.site_of_location(lec, location_fragment(&asset.location.value))
    }

    /// The buildings of `site` in `lec`, sorted.
    pub fn buildings_of_site(&self, lec: &str, site: &str) -> BTreeSet<&str> {
        self.building_sites
            .get(lec)
            .map(|buildings| {
                buildings
                    .iter()
                    .filter(|(_, building_site)| building_site.as_str() == site)
                    .map(|(building, _)| building.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The `#fragment` of a `location` URI (a building or site name), or the whole value if it
/// has none.
pub fn location_fragment(location: &str) -> &str {
    location
        .rsplit_once('#')
        .map_or(location, |(_, fragment)| fragment)
}
