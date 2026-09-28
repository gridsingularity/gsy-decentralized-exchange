use crate::auth::jwt::{JwtSecret, encode_jwt};
use crate::auth::model::UserModel;
use actix_web::{HttpResponse, Responder, post, web};
use bcrypt::verify;
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
) -> impl Responder {
    let user_model = match UserModel::new().await {
        Ok(m) => m,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let user = match user_model.find_by_username(&body.username).await {
        Ok(Some(u)) => u,
        Ok(None) => return HttpResponse::Unauthorized().finish(),
        Err(_) => return HttpResponse::InternalServerError().finish(),
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
