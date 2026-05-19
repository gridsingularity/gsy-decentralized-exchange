use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use gsy_offchain_primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilitySchema, PilotSiteSchema, SiteSchema,
};
use gsy_offchain_primitives::db_api_schema::tariff::TariffSchema;
use serde::Deserialize;

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

pub async fn post_community(community: Json<EnergyCommunitySchema>, db: DbRef) -> impl Responder {
    match db.get_ref().communities().insert(community.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_communities(db: DbRef) -> impl Responder {
    match db.get_ref().communities().get_all().await {
        Ok(communities) => HttpResponse::Ok().json(communities),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_site(site: Json<SiteSchema>, db: DbRef) -> impl Responder {
    match db.get_ref().sites().insert(site.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_sites(db: DbRef) -> impl Responder {
    match db.get_ref().sites().get_all().await {
        Ok(sites) => HttpResponse::Ok().json(sites),
        Err(e) => {
            tracing::error!("Failed to execute query: {:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

pub async fn post_facility(facility: Json<FacilitySchema>, db: DbRef) -> impl Responder {
    match db.get_ref().facilities().insert(facility.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub async fn get_facilities(db: DbRef) -> impl Responder {
    match db.get_ref().facilities().get_all().await {
        Ok(facilities) => HttpResponse::Ok().json(facilities),
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
