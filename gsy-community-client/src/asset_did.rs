//! Client for the `gsy-ewf-identity-server` asset/community DID sync endpoint.
//!
//! The community client is the *caller* in this relationship (plan §2.4, push model): it
//! already holds the FEDECOM ontology credentials, and — more importantly — it owns the
//! single definition of `deterministic_community_uuid` / `deterministic_area_uuid` /
//! `deterministic_area_hash` (`offchain_storage_connector::adapter`). Those ids are the join
//! keys for the whole forecast/order/market chain, so they are computed here and sent as
//! opaque values; the identity server never re-derives them.
//!
//! ## Wire contract
//!
//! Every struct below mirrors a TypeScript DTO in
//! `gsy-ewf-identity-server/src/assets/dto/`. The server boots its global `ValidationPipe`
//! with `forbidNonWhitelisted: true` (`gsy-ewf-identity-server/src/main.ts:10-16`), so a
//! field this crate sends that the DTO does not declare — including a snake_case spelling of
//! a declared camelCase field — makes the server reject the *entire* 590-subject payload with
//! a 400. `#[serde(rename_all = "camelCase")]` on the request structs is therefore load
//! bearing, and `tests/asset_did.rs` pins the emitted field names against a literal JSON
//! value rather than against a round-trip through these same structs.

use crate::constants::CommunityClientConstants;
use crate::offchain_storage_connector::adapter::{
    deterministic_areas, deterministic_community_uuid,
};
use crate::topology::ExternalCommunityTopology;
use anyhow::{Context, Result};
use gsy_offchain_primitives::db_api_schema::market::AssetType;
use gsy_offchain_primitives::utils::read_env_or;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// One community subject. Mirrors `CommunitySyncItem`
/// (`gsy-ewf-identity-server/src/assets/dto/asset-sync-request.dto.ts`).
///
/// `subject_uuid` and `community_uuid` are the same value on this item — both are carried
/// because the server treats the two subject types uniformly and keys its retirement scope
/// and `GET /asset-dids` filter on `communityUuid`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommunitySyncItem {
    /// `deterministic_community_uuid(community_name)`. Validated server-side by `@IsUUID`.
    pub subject_uuid: String,
    pub community_name: String,
    /// Equal to `subject_uuid` for a community subject.
    pub community_uuid: String,
}

/// One asset subject. Mirrors `AssetSyncItem`
/// (`gsy-ewf-identity-server/src/assets/dto/asset-sync-request.dto.ts`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSyncItem {
    /// `deterministic_area_uuid(community_name, asset_name)` — the canonical subject id and
    /// the natural join key against `AreaTopologySchema.area_uuid`.
    pub subject_uuid: String,
    pub asset_name: String,
    /// Deliberately the *same* `AssetType` the rest of the system stamps onto
    /// `AreaTopologySchema.area_type`, not a string built here: sharing the type means the
    /// wire value is produced by the same serde impl, so it cannot drift into a second
    /// spelling. Serialises to the bare variant name (`"SMART_METER"`, `"PV"`, ...). The
    /// server validates it with `@IsString`, not `@IsEnum`, so a future ontology asset type
    /// cannot 400 the whole sync.
    pub asset_type: AssetType,
    pub community_name: String,
    /// `deterministic_community_uuid(community_name)`.
    pub community_uuid: String,
    /// `h256_to_string(deterministic_area_hash(community_name, asset_name))`.
    pub area_hash: String,
}

/// Body of `POST /asset-dids/sync`. Mirrors `AssetSyncRequest`.
///
/// Both arrays are optional server-side but always sent here; the server rejects a payload
/// carrying neither with a 400, so [`AssetDidClient::sync`] refuses to send an empty one.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetSyncRequest {
    pub communities: Vec<CommunitySyncItem>,
    pub assets: Vec<AssetSyncItem>,
}

impl AssetSyncRequest {
    pub fn is_empty(&self) -> bool {
        self.communities.is_empty() && self.assets.is_empty()
    }

    pub fn subject_count(&self) -> usize {
        self.communities.len() + self.assets.len()
    }
}

/// One entry of the `subjectUuid -> did` map. Mirrors `SyncedSubjectDto`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncedSubject {
    pub subject_uuid: String,
    /// `"asset"` or `"community"`. Kept as a `String` so a subject type added server-side
    /// cannot make an otherwise successful sync fail to deserialise here.
    pub subject_type: String,
    pub did: String,
    #[serde(default)]
    pub registered_on_chain: bool,
}

/// Response of `POST /asset-dids/sync`. Mirrors `AssetSyncResponse`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetSyncResponse {
    pub created: u32,
    pub updated: u32,
    pub retired: u32,
    #[serde(default)]
    pub subjects: Vec<SyncedSubject>,
}

impl AssetSyncResponse {
    /// The `subjectUuid -> did` map the sync exists to produce.
    pub fn did_map(&self) -> HashMap<String, String> {
        self.subjects
            .iter()
            .map(|subject| (subject.subject_uuid.clone(), subject.did.clone()))
            .collect()
    }
}

/// Turn a fetched ontology topology into the two halves of a sync payload.
///
/// Pure, so the payload can be asserted in tests without an identity server or the
/// ontology. Both halves come from the *existing* deterministic helpers — the asset half via
/// [`deterministic_areas`], which is the same function that builds the market topology, so a
/// subject uuid here is by construction the `area_uuid` the market and every forecast use.
pub fn build_sync_payload(communities: &[ExternalCommunityTopology]) -> AssetSyncRequest {
    let mut request = AssetSyncRequest::default();

    for community in communities {
        let community_uuid = deterministic_community_uuid(&community.community_name);

        request.communities.push(CommunitySyncItem {
            subject_uuid: community_uuid.clone(),
            community_name: community.community_name.clone(),
            community_uuid: community_uuid.clone(),
        });

        for area in deterministic_areas(community) {
            request.assets.push(AssetSyncItem {
                subject_uuid: area.area_uuid,
                asset_name: area.name,
                asset_type: area.area_type,
                community_name: community.community_name.clone(),
                community_uuid: community_uuid.clone(),
                area_hash: area.area_hash,
            });
        }
    }

    request
}

/// HTTP client for the identity server's `/asset-dids` surface.
#[derive(Clone, Debug)]
pub struct AssetDidClient {
    client: Client,
    sync_url: String,
}

/// Build a reqwest client that sends the `x-api-key` header the identity server's
/// `ApiKeyGuard` requires, on every request.
///
/// Same shape as `adapter::authorized_storage_client` — including tolerating a key that is
/// not a valid header value rather than panicking at construction: the request then fails
/// with a 401 the sync loop logs, instead of taking the whole process down at start-up over
/// a mistyped env var. By design (plan §2.6) the key is the *same* `API_KEY` the off-chain
/// storage uses; the identity server reads `IDENTITY_API_KEY ?? API_KEY`, so a future key
/// split is a config change on both sides and not a code change here.
fn authorized_identity_client(api_key: &str) -> Client {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(api_key) {
        headers.insert("x-api-key", value);
    }
    Client::builder()
        .default_headers(headers)
        // Bounded, unlike the storage client: an identity server that accepts the connection
        // and then hangs would otherwise park the sync loop forever, and this loop's whole
        // contract is that it keeps ticking regardless of what the server does.
        .timeout(Duration::from_secs(
            CommunityClientConstants.HTTP_REQUEST_TIMEOUT_SEC,
        ))
        .connect_timeout(Duration::from_secs(
            CommunityClientConstants.HTTP_CONNECT_TIMEOUT_SEC,
        ))
        .build()
        .expect("Failed to build identity server HTTP client")
}

impl AssetDidClient {
    pub fn new(host: Option<String>, api_key: Option<String>) -> Self {
        let hostname = host.unwrap_or_else(|| CommunityClientConstants.IDENTITY_SERVER_URL.clone());
        // No dedicated constant by decision (plan §2.6, §4.4): the identity server shares
        // the off-chain storage's key, so this is the same `API_KEY` read — same var, same
        // default — as `adapter::authorized_storage_client`.
        let api_key = api_key.unwrap_or_else(|| read_env_or("API_KEY", "fedecom_user".to_string()));
        AssetDidClient {
            client: authorized_identity_client(&api_key),
            sync_url: format!("{}/asset-dids/sync", hostname.trim_end_matches('/')),
        }
    }

    pub fn sync_url(&self) -> &str {
        &self.sync_url
    }

    /// Bulk, idempotent upsert of every community and asset subject the ontology currently
    /// exposes. Re-posting an identical payload creates nothing and returns byte-identical
    /// DIDs; subjects absent from it, within the scope it covers, are retired, never deleted.
    ///
    /// Returns the whole response rather than just the `subjectUuid -> did` map (plan §4.4)
    /// so the caller can also log `created`/`updated`/`retired`; use
    /// [`AssetSyncResponse::did_map`] for the map itself.
    ///
    /// Every failure — unreachable server, timeout, 401, 400, unparseable body — is an `Err`.
    /// Nothing here panics, so the caller can log and carry on to the next tick.
    pub async fn sync(&self, request: &AssetSyncRequest) -> Result<AssetSyncResponse> {
        if request.is_empty() {
            // The server rejects an empty payload with a 400 ("a sync payload must carry at
            // least one community or one asset"); an empty ontology fetch is not an error
            // worth turning into one, so report success over nothing.
            return Ok(AssetSyncResponse::default());
        }

        let response = self
            .client
            .post(&self.sync_url)
            .json(request)
            .send()
            .await
            .with_context(|| format!("POST {} failed", self.sync_url))?;

        let status = response.status();
        if !status.is_success() {
            // Read the body: the server's 400 names the offending field, which is the only
            // thing that makes a DTO mismatch diagnosable from this side.
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "POST {} returned {}: {}",
                self.sync_url,
                status,
                body.chars().take(512).collect::<String>()
            );
        }

        response.json::<AssetSyncResponse>().await.with_context(|| {
            format!(
                "failed to deserialize the response of POST {}",
                self.sync_url
            )
        })
    }
}

impl Default for AssetDidClient {
    fn default() -> Self {
        Self::new(None, None)
    }
}
