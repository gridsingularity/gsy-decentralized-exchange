use crate::db::grid_topology_service::{NameConflict, TopologyEntry, TopologyService};
use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use anyhow::Result;
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use primitives::db_api_schema::tariff::TariffSchema;
use primitives::ewds::dto::{EwdsCommunityDto, EwdsFacilityDto, EwdsSiteDto};
use primitives::ewds::is_invalid_event;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct AssetQuery {
    uuid: Option<String>,
    facility_name: Option<String>,
}

pub async fn post_assets(assets: Json<Vec<AssetSchema>>, db: DbRef) -> impl Responder {
    match db.get_ref().assets().insert_assets(assets.to_vec()).await {
        Ok(ids) => HttpResponse::Ok().json(ids),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_assets(db: DbRef, query: Query<AssetQuery>) -> impl Responder {
    let service = db.get_ref().assets();
    if let Some(uuid) = &query.uuid {
        return match service.get_by_uuid(uuid).await {
            Ok(Some(asset)) => HttpResponse::Ok().json(asset),
            Ok(None) => HttpResponse::NotFound().finish(),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                HttpResponse::InternalServerError().finish()
            }
        };
    }
    if let Some(facility) = &query.facility_name {
        return match service.get_by_facility(facility).await {
            Ok(assets) => HttpResponse::Ok().json(assets),
            Err(e) => {
                tracing::error!("Failed to execute query: {:?}", e);
                HttpResponse::InternalServerError().finish()
            }
        };
    }
    match service.get_all().await {
        Ok(assets) => HttpResponse::Ok().json(assets),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_pilot_site(pilot: Json<PilotSiteSchema>, db: DbRef) -> impl Responder {
    match db.get_ref().pilot_sites().insert(pilot.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_pilot_sites(db: DbRef) -> impl Responder {
    match db.get_ref().pilot_sites().get_all().await {
        Ok(pilots) => HttpResponse::Ok().json(pilots),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_community(community: Json<EwdsCommunityDto>, db: DbRef) -> impl Responder {
    match EnergyCommunitySchema::try_from(community.into_inner()) {
        Ok(community) => store_response(
            db.get_ref().communities().store(vec![community]).await,
            EwdsCommunityDto::from,
        ),
        Err(error) => HttpResponse::BadRequest().body(format!("{error:#}")),
    }
}

pub async fn get_communities(db: DbRef) -> impl Responder {
    latest_response(db.get_ref().communities(), EwdsCommunityDto::from).await
}

pub async fn post_site(site: Json<EwdsSiteDto>, db: DbRef) -> impl Responder {
    match SiteSchema::try_from(site.into_inner()) {
        Ok(site) => store_response(
            db.get_ref().sites().store(vec![site]).await,
            EwdsSiteDto::from,
        ),
        Err(error) => HttpResponse::BadRequest().body(format!("{error:#}")),
    }
}

pub async fn get_sites(db: DbRef) -> impl Responder {
    latest_response(db.get_ref().sites(), EwdsSiteDto::from).await
}

pub async fn post_facility(facility: Json<EwdsFacilityDto>, db: DbRef) -> impl Responder {
    match FacilitySchema::try_from(facility.into_inner()) {
        Ok(facility) => store_response(
            db.get_ref().facilities().store(vec![facility]).await,
            EwdsFacilityDto::from,
        ),
        Err(error) => HttpResponse::BadRequest().body(format!("{error:#}")),
    }
}

pub async fn get_facilities(db: DbRef) -> impl Responder {
    latest_response(db.get_ref().facilities(), EwdsFacilityDto::from).await
}

/// 200 with the stored entry, 409 for a name that another entry uses, 400 for other invalid
/// data, and 500 otherwise.
fn store_response<T, D: Serialize>(
    result: Result<Vec<T>>,
    to_dto: impl Fn(T) -> D,
) -> HttpResponse {
    match result {
        Ok(stored) => match stored.into_iter().next() {
            Some(entry) => HttpResponse::Ok().json(to_dto(entry)),
            None => HttpResponse::InternalServerError().finish(),
        },
        Err(error) if error.downcast_ref::<NameConflict>().is_some() => {
            HttpResponse::Conflict().body(format!("{error:#}"))
        }
        Err(error) if is_invalid_event(&error) => {
            HttpResponse::BadRequest().body(format!("{error:#}"))
        }
        Err(error) => {
            tracing::error!("Failed to store: {error:#}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 200 with the latest entries of `service`.
async fn latest_response<T: TopologyEntry, D: Serialize>(
    service: TopologyService<T>,
    to_dto: impl Fn(T) -> D,
) -> HttpResponse {
    match service.get().await {
        Ok(entries) => HttpResponse::Ok().json(entries.into_iter().map(to_dto).collect::<Vec<_>>()),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_tariff(tariff: Json<TariffSchema>, db: DbRef) -> impl Responder {
    match db.get_ref().tariffs().insert(tariff.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_tariffs(db: DbRef) -> impl Responder {
    match db.get_ref().tariffs().get_all().await {
        Ok(tariffs) => HttpResponse::Ok().json(tariffs),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
