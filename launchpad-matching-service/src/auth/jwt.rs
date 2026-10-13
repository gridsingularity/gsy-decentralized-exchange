use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use futures_util::future::{Ready, ready};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct JwtSecret(pub String);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

pub fn encode_jwt(
    username: &str,
    secret: &str,
    expiry_hours: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: username.to_string(),
        iat: now,
        exp: now + (expiry_hours as usize) * 3600,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

impl FromRequest for Claims {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let secret = match req.app_data::<web::Data<JwtSecret>>() {
            Some(s) => s.0.clone(),
            None => {
                return ready(Err(actix_web::error::ErrorInternalServerError(
                    "JWT secret not configured",
                )));
            }
        };

        let auth_header = match req.headers().get("Authorization") {
            Some(h) => h,
            None => {
                return ready(Err(actix_web::error::ErrorUnauthorized(
                    "Missing Authorization header",
                )));
            }
        };

        let auth_str = match auth_header.to_str() {
            Ok(s) => s,
            Err(_) => {
                return ready(Err(actix_web::error::ErrorUnauthorized(
                    "Invalid Authorization header",
                )));
            }
        };

        if !auth_str.starts_with("Bearer ") {
            return ready(Err(actix_web::error::ErrorUnauthorized(
                "Authorization header must use Bearer scheme",
            )));
        }

        let token = &auth_str[7..];
        match decode_jwt(token, &secret) {
            Ok(claims) => ready(Ok(claims)),
            Err(_) => ready(Err(actix_web::error::ErrorUnauthorized(
                "Invalid or expired token",
            ))),
        }
    }
}
