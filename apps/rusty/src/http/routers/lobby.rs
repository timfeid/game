use async_stream::stream;
use futures::Stream;
use futures::StreamExt;
use rspc::internal::UnbuiltProcedureBuilder;
use rspc::{Router, RouterBuilder};
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::AppError;
use crate::http::controllers::lobby::LobbyChatArgs;
use crate::services::jwt::JwtService;
use crate::{http::controllers::lobby::LobbyController, lobby::lobby::LobbyData, Ctx};

pub fn create_lobby_router() -> rspc::RouterBuilder<Ctx> {
    Router::<Ctx>::new()
        .mutation("chat", |t| {
            t(|ctx, args: LobbyChatArgs| async move { Ok(LobbyController::chat(ctx, args).await?) })
        })
        .mutation("create", |t| {
            t(|ctx, _: Vec<String>| async move { Ok(LobbyController::create(ctx).await?) })
        })
        .subscription("subscribe", |t| {
            t(|ctx, (join_code, access_token): (String, String)| {
                LobbyController::subscribe(ctx, join_code, access_token)
            })
        })
}
