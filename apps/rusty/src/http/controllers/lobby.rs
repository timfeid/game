use std::sync::Arc;

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{Mutex, MutexGuard};
use tokio_stream::StreamExt;

use crate::{
    error::{AppError, AppResult},
    game::FrontendTarget,
    lobby::{
        lobby::{DeckSelector, Lobby, LobbyChat, LobbyData},
        manager::{LobbyCommand, LobbyManager},
    },
    services::jwt::{Claims, JwtService},
    Ctx,
};

#[derive(Type, Serialize, Deserialize)]
pub struct RespondMandatoryAbility {
    pub code: String,
    pub target: Option<FrontendTarget>,
    pub ability_id: String,
}

#[derive(Type, Serialize, Deserialize)]
pub struct RespondOptionalAbility {
    pub code: String,
    pub target: Option<FrontendTarget>,
    pub ability_id: String,
    pub response: bool,
}

#[derive(Type, Serialize, Deserialize)]
pub struct ActionCardArgs {
    pub trigger_id: String,
    pub code: String,
    pub player_index: i32,
    pub in_play_index: i32,
    pub target: Option<FrontendTarget>,
}

#[derive(Type, Serialize, Deserialize)]
pub struct SelectDeckArgs {
    pub code: String,
    pub deck: DeckSelector,
}

#[derive(Type, Serialize, Deserialize)]
pub struct PlayCardArgs {
    pub code: String,
    pub in_hand_index: i32,
    pub target: Option<FrontendTarget>,
}

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

fn personalize_lobby_data_for_player(command: &mut LobbyCommand, user_id: &str) {
    if let LobbyCommand::Updated(lobby_data) = command {
        // For each player in the game state
        for (id, player_state) in &mut lobby_data.game_state.players {
            if id != user_id {
                // Hide private information
                player_state.hand.clear();
            }
        }
    }
}

pub struct LobbyController {}
impl LobbyController {
    pub async fn select_deck(ctx: Ctx, args: SelectDeckArgs) -> AppResult<()> {
        let code = args.code;
        let deck = args.deck;
        let user = ctx.required_user()?;

        // Step 1: Get the lobby instance from the lobby manager and release the lock
        let l = Arc::clone(&ctx.lobby_manager);
        let lobby = l
            .get_lobby(&code)
            .await
            .map_err(|_| AppError::BadRequest("No such lobby".to_string()))?;

        lobby.lock().await.select_deck(user, deck).await;

        ctx.lobby_manager.notify_lobby(&code).await.ok();

        Ok(())
    }

    pub async fn ready(ctx: Ctx, code: String) -> AppResult<()> {
        let user = ctx.required_user()?;

        // Step 1: Get the lobby instance from the lobby manager and release the lock
        let l = Arc::clone(&ctx.lobby_manager);
        let lobby = l
            .get_lobby(&code)
            .await
            .map_err(|_| AppError::BadRequest("No such lobby".to_string()))?;

        lobby.lock().await.ready(user).await;

        ctx.lobby_manager.notify_lobby(&code).await.ok();

        Ok(())
    }

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

    pub(crate) async fn turn(ctx: Ctx, join_code: String) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager
            .advance_turn(&join_code, user)
            .await
            .ok_or(AppError::BadRequest(
                "Bad lobby id or not your turn".to_string(),
            ))?;

        Ok(())
    }

    pub(crate) async fn action_card(ctx: Ctx, args: ActionCardArgs) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager.action_card(args, user).await?;

        Ok(())
    }

    pub(crate) async fn attach_card(ctx: Ctx, args: ActionCardArgs) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager.attach_card(args, user).await?;

        Ok(())
    }

    pub(crate) async fn respond_optional_ability(
        ctx: Ctx,
        args: RespondOptionalAbility,
    ) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager
            .respond_optional_player_ability(args, user)
            .await?;

        Ok(())
    }

    pub(crate) async fn play_card(ctx: Ctx, args: PlayCardArgs) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager.play_card(args, user).await?;

        Ok(())
    }

    pub(crate) async fn join(ctx: Ctx, join_code: String) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager
            .join_lobby(&join_code, user)
            .await
            .ok_or(AppError::BadRequest("Bad lobby id".to_string()))?;
        ctx.lobby_manager.notify_lobby(&join_code).await.ok();

        Ok(())
    }

    pub(crate) async fn chat(ctx: Ctx, args: LobbyChatArgs) -> AppResult<()> {
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

        Ok(())
    }

    pub(crate) fn subscribe(
        ctx: Ctx,
        join_code: String,
        access_token: String,
    ) -> impl Stream<Item = LobbyCommand> + Send + 'static {
        let manager = Arc::clone(&ctx.lobby_manager);
        let user_claims = JwtService::decode(&access_token).unwrap().claims;

        let async_stream = stream! {
            if let Ok(mut post_stream) = manager.subscribe_to_lobby_updates(join_code, access_token).await {
                while let Some(mut lobby_data) = post_stream.next().await {
                        match &lobby_data {
                            LobbyCommand::AskExecuteAbility(ability_details) => {
                                if ability_details.player_id == user_claims.sub.clone() {
                                    yield lobby_data;
                                }
                            },
                            _ => {
                                personalize_lobby_data_for_player(&mut lobby_data, &user_claims.sub);

                                yield lobby_data;
                            }
                        }


                }
            }
        };
        let async_stream = async_stream;
        async_stream
    }

    pub(crate) async fn respond_mandatory_ability(
        ctx: Ctx,
        args: RespondMandatoryAbility,
    ) -> AppResult<()> {
        let user = ctx.required_user()?;
        ctx.lobby_manager
            .respond_mandatory_player_ability(args, user)
            .await?;

        Ok(())
    }
}
