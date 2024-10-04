use async_stream::stream;
use futures::Stream;
use futures::StreamExt;
use rspc::internal::UnbuiltProcedureBuilder;
use rspc::{Router, RouterBuilder};
use serde::Deserialize;
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::AppError;
use crate::http::controllers::lobby::ActionCardArgs;
use crate::http::controllers::lobby::LobbyChatArgs;
use crate::http::controllers::lobby::PlayCardArgs;
use crate::http::controllers::lobby::SelectDeckArgs;
use crate::services::jwt::JwtService;
use crate::{http::controllers::lobby::LobbyController, lobby::lobby::LobbyData, Ctx};

pub fn create_lobby_router() -> rspc::RouterBuilder<Ctx> {
    Router::<Ctx>::new()
        .mutation("chat", |t| {
            t(|ctx, args: LobbyChatArgs| async move { Ok(LobbyController::chat(ctx, args).await?) })
        })
        .mutation("turn", |t| {
            t(|ctx, code: String| async move { Ok(LobbyController::turn(ctx, code).await?) })
        })
        .mutation("attach_card", |t| {
            t(|ctx, args: ActionCardArgs| async move {
                Ok(LobbyController::attach_card(ctx, args).await?)
            })
        })
        .mutation("action_card", |t| {
            t(|ctx, args: ActionCardArgs| async move {
                Ok(LobbyController::action_card(ctx, args).await?)
            })
        })
        .mutation("play_card", |t| {
            t(
                |ctx, args: PlayCardArgs| async move {
                    Ok(LobbyController::play_card(ctx, args).await?)
                },
            )
        })
        .mutation("join", |t| {
            t(|ctx, code: String| async move { Ok(LobbyController::join(ctx, code).await?) })
        })
        .mutation("select_deck", |t| {
            t(|ctx, args: SelectDeckArgs| async move {
                Ok(LobbyController::select_deck(ctx, args).await?)
            })
        })
        .mutation("ready", |t| {
            t(|ctx, code: String| async move { Ok(LobbyController::ready(ctx, code).await?) })
        })
        .mutation("create", |t| {
            t(|ctx, _: Vec<String>| async move { Ok(LobbyController::create(ctx).await?) })
        })
        .subscription("subscribe", |t| {
            t(|ctx, (code, access_token): (String, String)| {
                LobbyController::subscribe(ctx, code, access_token)
            })
        })
}
