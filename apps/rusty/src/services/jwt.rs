use std::ops::Add;

use jsonwebtoken::{
    decode, encode, get_current_timestamp, Algorithm, DecodingKey, EncodingKey, Header, TokenData,
    Validation,
};

use rspc::Error;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    models::user::User,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub jti: Option<String>,
    pub exp: u64,
}

pub struct JwtService {}

impl JwtService {
    pub fn decode(token: &str) -> AppResult<jsonwebtoken::TokenData<Claims>> {
        return Ok(decode(
            token,
            &DecodingKey::from_rsa_pem(include_bytes!("../../jwt.public.pem"))
                .map_err(|e| AppError::InternalServerError(e.to_string()))?,
            &Validation::new(Algorithm::RS256),
        )
        .map_err(|e| AppError::InternalServerError(e.to_string()))?);
    }

    pub fn create_for_user(user: &User, jti: Option<String>) -> AppResult<String> {
        let is_access_token = &jti.is_none();
        let claims = Claims {
            jti,
            sub: user.get_id().to_string(),
            exp: get_current_timestamp().add(match is_access_token {
                &true => 1,
                &false => 604800,
            }),
        };

        let response = encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &EncodingKey::from_rsa_pem(include_bytes!("../../jwt.private.pem"))
                .map_err(|e| AppError::InternalServerError(e.to_string()))?,
        );

        Ok(response.map_err(|e| AppError::InternalServerError(e.to_string()))?)
    }
}
