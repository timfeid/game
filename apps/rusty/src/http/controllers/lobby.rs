use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{Mutex, MutexGuard};
use tokio_stream::StreamExt;

use crate::{
    error::{AppError, AppResult},
    lobby::{
        lobby::{Lobby, LobbyChat, LobbyData},
        manager::LobbyManager,
    },
    services::jwt::{Claims, JwtService},
    Ctx,
};

#[derive(Deserialize, Type)]
pub struct ListAccountArgs {}

#[derive(Serialize, Deserialize, Type)]
pub struct CreateAccountArgs {
    url: String,
    issuer: String,
    username: String,
}

#[derive(Serialize, Deserialize, Type)]
pub struct LobbyChatArgs {
    lobby_id: String,
    text: String,
}

fn personalize_lobby_data_for_player(lobby_data: &mut LobbyData, user_id: &str) {
    // For each player in the game state
    for player_state in &mut lobby_data.game_state.players {
        if player_state.user_id != user_id {
            // Hide private information
            player_state.hand.clear();
        }
    }
}

pub struct LobbyController {}
impl LobbyController {
    pub async fn create(ctx: Ctx) -> AppResult<LobbyData> {
        let user = ctx.required_user()?;
        let code = ctx.lobby_manager.create_lobby(user).await?;
        let lobby = ctx
            .lobby_manager
            .get_lobby(&code)
            .await
            .map_err(|x| AppError::BadRequest("No such lobby".to_string()))?;
        let data = lobby.lock().await.data.clone();

        Ok(data)
    }

    pub(crate) async fn chat(ctx: Ctx, args: LobbyChatArgs) -> AppResult<bool> {
        let user = ctx.required_user()?;
        let lobby = ctx
            .lobby_manager
            .get_lobby(&args.lobby_id)
            .await
            .map_err(|x| AppError::BadRequest("Bad lobby id".to_string()))?;
        // let data = &lobby.lock().await.data;

        // println!("adding message to lobby {} {:?}", data.join_code, lobby);

        lobby.lock().await.message(user, args.text);
        println!("added, notifying lobby");
        // lobby.lock().await.message(user, args.text);
        ctx.lobby_manager.notify_lobby(&args.lobby_id).await.ok();
        println!("done.");

        Ok(true)
    }

    pub(crate) fn subscribe(
        ctx: Ctx,
        join_code: String,
        access_token: String,
    ) -> impl Stream<Item = LobbyData> + Send + 'static {
        let manager = Arc::clone(&ctx.lobby_manager);
        let user_claims = JwtService::decode(&access_token).unwrap().claims;

        stream! {
            if let Ok(mut post_stream) = manager.subscribe_to_lobby_updates(join_code, access_token).await {
                while let Some(mut lobby_data) = post_stream.next().await {
                    personalize_lobby_data_for_player(&mut lobby_data, &user_claims.sub);

                    yield lobby_data;
                }
            }
        }
    }
}
