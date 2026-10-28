use crate::db::DbRef;
use actix_web::{web::Json, web::Query, HttpResponse, Responder};
use primitives::db_api_schema::grid_topology::{
    AssetSchema, EnergyCommunitySchema, FacilityMarketValidationRequest, FacilitySchema,
    PilotSiteSchema, SiteSchema,
};
use primitives::db_api_schema::tariff::TariffSchema;
use primitives::utils::normalize_community_id;
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
    let mut community = community.into_inner();
    community.community_id = match normalize_community_id(&community.community_id) {
        Ok(id) => id,
        Err(error) => return HttpResponse::BadRequest().body(error.to_string()),
    };
    match db
        .get_ref()
        .communities()
        .upsert(community)
        .await
    {
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
    match db.get_ref().sites().upsert(site.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(error) => {
            tracing::error!("Site upsert failed: {error:#}");
            HttpResponse::InternalServerError().json(primitives::ewds::dto::EwdsErrorPayload {
                code: "STORAGE_UNAVAILABLE".to_string(),
                message: "Site could not be saved".to_string(),
            })
        }
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
    match db.get_ref().facilities().upsert(facility.to_owned()).await {
        Ok(saved) => HttpResponse::Ok().json(saved),
        Err(e) => {
            if e.to_string().contains("E11000") || format!("{e:#}").contains("E11000") {
                HttpResponse::Conflict().body("facility already exists")
            } else {
                tracing::error!("Facility upsert failed: {e:#}");
                HttpResponse::InternalServerError().finish()
            }
        }
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

pub async fn validate_facility_markets(
    requests: Json<Vec<FacilityMarketValidationRequest>>,
    db: DbRef,
) -> impl Responder {
    match db.validate_facility_markets(&requests).await {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(error) => {
            tracing::error!("Facility validation failed: {error:#}");
            HttpResponse::InternalServerError().json(primitives::ewds::dto::EwdsErrorPayload {
                code: "VALIDATION_UNAVAILABLE".to_string(),
                message: "Facility validation could not read the stored topology".to_string(),
            })
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
