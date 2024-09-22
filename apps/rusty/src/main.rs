use std::{fs::write, future::IntoFuture, path::PathBuf, sync::Arc};

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        request::Parts,
        Method,
    },
    routing::get,
};
use database::create_connection;
use error::{AppError, AppResult};
use http::routers::create_router;
use lobby::manager::LobbyManager;
use services::jwt::{Claims, JwtService};
use sqlx::{Executor, Pool, Postgres};
use tokio::sync::Mutex;
use totp_rs::{Algorithm, Secret, TOTP};

use rspc::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};

async fn create_pool() -> Arc<Pool<Postgres>> {
    let database_url = dotenv::var("DATABASE_URL").unwrap();
    create_connection(&database_url).await
}

async fn create_lobby_manager() -> Arc<LobbyManager> {
    let manager = LobbyManager::new("redis://127.0.0.1/").await.unwrap();
    Arc::new(manager)
}

async fn create_app() -> axum::Router {
    let router = create_router();
    let allowed_headers = [CONTENT_TYPE, AUTHORIZATION];
    let allowed_methods = [Method::GET, Method::POST, Method::OPTIONS];
    let pool = create_pool().await;
    let lobby_manager = create_lobby_manager().await;

    axum::Router::new()
        .route("/", get(|| async { "Hello 'rspc'!" }))
        .nest(
            "/rspc",
            rspc_axum::endpoint(router, |parts: Parts| Ctx::new(pool, parts, lobby_manager)),
        )
        .layer(
            CorsLayer::new()
                .allow_methods(allowed_methods)
                .allow_headers(allowed_headers)
                .allow_origin(AllowOrigin::mirror_request())
                .allow_credentials(true),
        )
}

mod database;
mod error;
mod game;
mod http;
mod lobby;
mod models;
mod services;

#[derive(Debug)]
pub struct Ctx {
    pub pool: Arc<Pool<Postgres>>,
    user: Option<Claims>,
    lobby_manager: Arc<LobbyManager>,
}

impl Ctx {
    pub fn new(pool: Arc<Pool<Postgres>>, parts: Parts, lobby_manager: Arc<LobbyManager>) -> Ctx {
        // println!("{:?}", parts.headers);
        let user = match parts.headers.get("Authorization") {
            Some(bearer) => JwtService::decode(bearer.to_str().unwrap_or_default())
                .and_then(|r| Ok(r.claims))
                .ok(),
            None => None,
        };

        Ctx {
            pool,
            user,
            lobby_manager,
        }
    }

    pub fn required_user(self: &Ctx) -> AppResult<&Claims> {
        // println!("{:?}", self);
        if self.user.is_none() {
            return Err(AppError::Unauthorized);
        }
        // Err(AppError::Unauthorized)
        Ok(self.user.as_ref().unwrap())
    }
}

// async fn handler(context: Ctx) {
//     let account = Account::find(&context.pool.clone(), "test".to_string())
//         .await
//         .expect("hi");

//     let totp = &account.get_current_code().expect("hi");
//     let token = totp.generate_current().unwrap();

//     println!("{:?}, token: {}", account, token);
// }

#[tokio::main]
async fn main() {
    // handler(context).await;

    let app = create_app().await;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    // let totp = TOTP::new(
    //     Algorithm::SHA1,
    //     6,
    //     1,
    //     30,
    //     Secret::Raw("TestSecretSuperSecret".as_bytes().to_vec())
    //         .to_bytes()
    //         .unwrap(),
    //     Some("test".to_string()),
    //     "dazed".to_string(),
    // )
    // .unwrap();
    // let token = totp.generate_current().unwrap();
    // println!("{}", token);
    // write("test.png", totp.get_qr_png().expect("unable to create png")).expect("Unable to write");
}

#[test]
fn tst() {
    let totp_real = TOTP::from_url("otpauth://totp/DigitalOcean:admin@timfeid.com?algorithm=SHA1&digits=6&issuer=DigitalOcean&period=30&secret=KEMMHM7H4IFX6FMY2Y7X4SUPAI3S56XV".to_string()).unwrap();
    println!("{}", totp_real.generate_current().unwrap());
    let secret = Secret::Encoded("KEMMHM7H4IFX6FMY2Y7X4SUPAI3S56XV".to_string());
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("test".to_string()),
        "dazed".to_string(),
    )
    .unwrap();
    let token = totp.generate_current().unwrap();
    println!("{}", token);
}
