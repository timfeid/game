use bcrypt::verify;
use sqlx::{Pool, Postgres};

use rspc::{Router, RouterBuilder};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    error::{AppError, AppResult},
    models::user::User,
    services::jwt::JwtService,
    Ctx,
};

#[derive(Type, Serialize)]
pub struct AuthResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub success: bool,
}

impl AuthResponse {
    async fn new(pool: &Pool<Postgres>, user: User) -> AppResult<AuthResponse> {
        let jti = user.create_refresh_token(pool).await?;

        Ok(AuthResponse {
            access_token: Some(JwtService::create_for_user(&user, None)?),
            refresh_token: Some(JwtService::create_for_user(&user, Some(jti))?),
            success: true,
        })
    }
}

#[derive(Type, Deserialize)]
pub struct LoginArgs {
    username: String,
    password: String,
}

pub struct AuthenticationController {}
impl AuthenticationController {
    pub async fn login(ctx: Ctx, args: LoginArgs) -> AppResult<AuthResponse> {
        if let Ok(user) = User::find(&ctx.pool, &args.username).await {
            if user.verify_password(&args.password) {
                return AuthResponse::new(&ctx.pool, user).await;
            }
            println!("invalid password");
        }

        Ok(AuthResponse {
            access_token: None,
            refresh_token: None,
            success: false,
        })
    }

    pub async fn refresh_token(ctx: Ctx, token: String) -> AppResult<AuthResponse> {
        let details = JwtService::decode(&token)
            .map_err(|_| AppError::BadRequest("Invalid token".to_owned()))?;

        if let Ok(user) = User::find_by_refresh_token(
            &ctx.pool,
            (&details.claims.sub, &details.claims.jti.unwrap_or_default()),
        )
        .await
        {
            return AuthResponse::new(&ctx.pool, user).await;
        }

        Ok(AuthResponse {
            access_token: None,
            refresh_token: None,
            success: false,
        })
    }

    pub async fn me(ctx: Ctx) -> AppResult<String> {
        Ok("hi".to_string())
    }
}
