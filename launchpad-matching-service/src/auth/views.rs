use crate::api::views::database_error_response;
use crate::auth::jwt::{JwtSecret, encode_jwt};
use crate::auth::model::UserModel;
use actix_web::{HttpResponse, Responder, post, web};
use bcrypt::verify;
use mongodb::Database;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TokenRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
}

#[post("/token")]
pub async fn get_token(
    body: web::Json<TokenRequest>,
    jwt_secret: web::Data<JwtSecret>,
    db: web::Data<Database>,
) -> impl Responder {
    let user_model = UserModel::from_database(db.get_ref().clone());

    let user = match user_model.find_by_username(&body.username).await {
        Ok(Some(u)) => u,
        Ok(None) => return HttpResponse::Unauthorized().finish(),
        Err(e) => {
            eprintln!("Failed to fetch user from MongoDB: {:?}", e);
            return database_error_response();
        }
    };

    match verify(&body.password, &user.password_hash) {
        Ok(true) => {}
        _ => return HttpResponse::Unauthorized().finish(),
    }
    match encode_jwt(&body.username, &jwt_secret.0, 24) {
        Ok(token) => HttpResponse::Ok().json(TokenResponse {
            access_token: token,
            token_type: "bearer".to_string(),
        }),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
