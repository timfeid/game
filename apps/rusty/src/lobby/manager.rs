use futures::stream::StreamExt;
use futures::Stream;
use redis::aio::PubSub;
use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use super::lobby::{Lobby, LobbyData};
use crate::error::{AppError, AppResult};
use crate::game::action::{CardAction, CardRequiredTarget};
use crate::game::card::Card;
use crate::game::effects::EffectTarget;
use crate::game::mana::ManaType;
use crate::game::player::Player;
use crate::game::stat::Stats;
use crate::game::{ActionType, CardWithDetails, FrontendTarget, Game, GameStatus, PlayerStatus};
use crate::http::controllers::lobby::{
    ActionCardArgs, PlayCardArgs, RespondMandatoryAbility, RespondOptionalAbility,
};
use crate::services::jwt::{Claims, JwtService};

#[derive(Clone)]
pub struct LobbyManager {
    redis_client: Arc<redis::Client>,
    lobbies: Arc<Mutex<HashMap<String, Arc<Mutex<Lobby>>>>>,
}

#[derive(Type, Deserialize, Clone, Serialize, Debug)]
pub struct LobbyTurnMessage {
    pub messages: Vec<String>,
}

#[derive(Type, Deserialize, Clone, Serialize, Debug)]
pub struct AbilityDetails {
    pub mana_cost: Vec<ManaType>,
    pub required_target: CardRequiredTarget,
    pub description: String,
    pub action_type: ActionType,
    pub show: bool,
    pub id: String,
    pub meets_requirements: bool,
}

#[derive(Type, Deserialize, Clone, Serialize, Debug)]
pub struct ExecuteAbility {
    card: CardWithDetails,
    details: AbilityDetails,
    pub player_id: String,
}

impl ExecuteAbility {
    pub fn new(
        player_id: String,
        card: CardWithDetails,
        action_type: ActionType,
        mana_cost: Vec<ManaType>,
        required_target: CardRequiredTarget,
        description: String,
        id: String,
        meets_requirements: bool,
    ) -> Self {
        Self {
            card,
            details: AbilityDetails {
                mana_cost,
                required_target,
                description,
                action_type,
                show: true,
                id,
                meets_requirements,
            },
            player_id,
        }
    }
}

#[derive(Type, Clone, Deserialize, Serialize, Debug)]
#[specta(export = false)]
pub enum LobbyCommand {
    Updated(LobbyData),
    Messages(Vec<String>),
    DebugMessage(String),
    TurnMessages(LobbyTurnMessage),
    AskExecuteAbility(ExecuteAbility),
    MandatoryExecuteAbility(ExecuteAbility),
}

impl std::fmt::Debug for LobbyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LobbyManager")
            .field("lobbies", &self.lobbies)
            .finish()
    }
}

impl LobbyManager {
    pub async fn create_lobby(self: &Arc<Self>, user: &Claims) -> AppResult<String> {
        let mut lobbies = self.lobbies.lock().await;
        let lobby = Lobby::new(user).await;
        let lobby_id = lobby.data.join_code.clone();
        let lobby_manager_weak = Arc::downgrade(self);
        let lobby_id_clone = lobby_id.clone();
        let game_arc_clone = lobby.cloned_game().await;

        lobbies.insert(lobby_id.clone(), Arc::new(Mutex::new(lobby)));

        tokio::spawn(async move {
            let rx = {
                let game = game_arc_clone.lock().await;
                game.broadcast_sender
                    .as_ref()
                    .map(|sender| sender.subscribe())
            };

            if let Some(mut rx) = rx {
                while let Ok(message) = rx.recv().await {
                    if let Some(lobby_manager) = lobby_manager_weak.upgrade() {
                        if let Some(command) = message {
                            lobby_manager
                                .send_command(&lobby_id_clone, command)
                                .await
                                .ok();
                        } else {
                            lobby_manager.notify_lobby(&lobby_id_clone).await.ok();
                        }
                    } else {
                        // The LobbyManager has been dropped; exit the task
                        break;
                    }
                }
            }
        });

        Ok(lobby_id)
    }

    pub async fn get_lobby(&self, join_code: &String) -> AppResult<Arc<Mutex<Lobby>>> {
        // Lock the `lobbies` to get the lobby reference.
        let lobbies = self.lobbies.lock().await;

        // Find the specific lobby or return an error if it doesn't exist.
        let lobby = lobbies
            .get(join_code)
            .ok_or(AppError::BadRequest("Lobby not found".to_owned()))?
            .clone();

        Ok(lobby)
    }

    // Stream game updates from Redis for a specific lobby
    pub async fn subscribe_to_lobby_updates(
        &self,
        lobby_id: String,
        access_token: String,
    ) -> AppResult<impl Stream<Item = LobbyCommand>> {
        let user = JwtService::decode(&access_token).or(Err(AppError::Unauthorized))?;
        let (tx, rx) = mpsc::channel::<LobbyCommand>(100);

        println!("{:?} has joined!", user.claims);

        // Clone redis client so it can be passed into the async block.
        let redis_client = Arc::clone(&self.redis_client);

        // Spawn the Redis subscription in a new task, but keep the mutex scope minimal
        tokio::spawn(async move {
            if let Err(e) = Self::handle_lobby_subscription(redis_client, lobby_id, tx).await {
                eprintln!("Error in subscription: {:?}", e);
            }
        });

        // Return the receiver stream
        Ok(ReceiverStream::new(rx))
    }

    // This function handles the subscription logic to keep the original method clean
    async fn handle_lobby_subscription(
        redis_client: Arc<redis::Client>,
        lobby_id: String,
        tx: mpsc::Sender<LobbyCommand>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut pubsub_conn = redis_client.get_async_pubsub().await?;
        pubsub_conn.subscribe(&lobby_id).await?;

        let mut pubsub_stream = pubsub_conn.on_message();
        while let Some(message) = pubsub_stream.next().await {
            let payload: String = message.get_payload()?;
            if let Ok(game) = serde_json::from_str::<LobbyCommand>(&payload) {
                if tx.send(game).await.is_err() {
                    eprintln!("Receiver dropped");
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn join_lobby(&self, lobby_id: &str, user: &Claims) -> Option<()> {
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map.get(lobby_id)?;
            lobby.lock().await.join(user).await;
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(lobby_id).await.ok();

        Some(())
    }

    pub async fn advance_turn(&self, lobby_id: &str, user: &Claims) -> Option<()> {
        // TODO: check if current turn is user's turn..
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map.get(lobby_id)?;
            lobby.lock().await.advance_turn().await;
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(lobby_id).await.ok();

        Some(())
    }

    pub async fn convert(
        target: Option<FrontendTarget>,
        lobby: &Arc<Mutex<Lobby>>,
    ) -> Option<EffectTarget> {
        match target {
            Some(target) => match target {
                FrontendTarget::Card(frontend_card_target) => match frontend_card_target.pile {
                    crate::game::FrontendPileName::Hand => {
                        let player = Arc::clone(
                            &lobby.lock().await.cloned_game().await.lock().await.players
                                [frontend_card_target.player_index as usize],
                        );
                        let card = &player.lock().await.cards_in_hand
                            [frontend_card_target.card_index as usize];
                        Some(EffectTarget::Card(Arc::clone(&card)))
                    }
                    crate::game::FrontendPileName::Play => {
                        let player = Arc::clone(
                            &lobby.lock().await.cloned_game().await.lock().await.players
                                [frontend_card_target.player_index as usize],
                        );
                        let card = &player.lock().await.cards_in_play
                            [frontend_card_target.card_index as usize];
                        Some(EffectTarget::Card(Arc::clone(&card)))
                    }
                    crate::game::FrontendPileName::Spell => {
                        let player = Arc::clone(
                            &lobby.lock().await.cloned_game().await.lock().await.players
                                [frontend_card_target.player_index as usize],
                        );
                        let card =
                            &player.lock().await.spells[frontend_card_target.card_index as usize];
                        Some(EffectTarget::Card(Arc::clone(&card)))
                    }
                },
                FrontendTarget::Player(player_index) => Some(EffectTarget::Player(Arc::clone(
                    &lobby.lock().await.cloned_game().await.lock().await.players
                        [player_index as usize],
                ))),
            },
            None => None,
        }
    }

    pub async fn attach_card(&self, args: ActionCardArgs, user: &Claims) -> AppResult<()> {
        let lobby_id = args.code;
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map
                .get(&lobby_id)
                .ok_or_else(|| AppError::BadRequest("Bad lobby".to_string()))?;
            let target = Self::convert(args.target, lobby).await;
            lobby
                .lock()
                .await
                .attach_card(
                    args.player_index as usize,
                    args.in_play_index as usize,
                    target,
                )
                .await?;
            println!("attached card, notifying lobby");
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(&lobby_id).await.ok();

        Ok(())
    }

    pub async fn action_card(&self, args: ActionCardArgs, user: &Claims) -> AppResult<()> {
        let lobby_id = args.code;
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map
                .get(&lobby_id)
                .ok_or_else(|| AppError::BadRequest("Bad lobby".to_string()))?;
            let target = { Self::convert(args.target, lobby).await };
            lobby
                .lock()
                .await
                .action_card(
                    args.player_index as usize,
                    args.in_play_index as usize,
                    target,
                    args.trigger_id,
                )
                .await?;
            println!("actioned card, notifying lobby");
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(&lobby_id).await.ok();

        Ok(())
    }

    pub async fn respond_mandatory_player_ability(
        &self,
        args: RespondMandatoryAbility,
        user: &Claims,
    ) -> AppResult<()> {
        let lobby_id = args.code;
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map
                .get(&lobby_id)
                .ok_or_else(|| AppError::BadRequest("Bad lobby".to_string()))?;
            let target = { Self::convert(args.target, lobby).await };
            let player = lobby
                .lock()
                .await
                .data
                .game_state
                .players
                .get(&user.sub)
                .unwrap()
                .player
                .clone();

            lobby
                .lock()
                .await
                .respond_mandatory_player_ability(args.ability_id, player, target)
                .await?;
            println!("actioned card, notifying lobby");
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(&lobby_id).await.ok();

        Ok(())
    }

    pub async fn respond_optional_player_ability(
        &self,
        args: RespondOptionalAbility,
        user: &Claims,
    ) -> AppResult<()> {
        let lobby_id = args.code;
        {
            let hash_map = self.lobbies.lock().await;
            let lobby = hash_map
                .get(&lobby_id)
                .ok_or_else(|| AppError::BadRequest("Bad lobby".to_string()))?;
            let target = { Self::convert(args.target, lobby).await };
            let player = lobby
                .lock()
                .await
                .data
                .game_state
                .players
                .get(&user.sub)
                .unwrap()
                .player
                .clone();

            lobby
                .lock()
                .await
                .respond_optional_player_ability(args.ability_id, player, target, args.response)
                .await?;
            println!("actioned card, notifying lobby");
        }
        // lobby.lock().await.message(user, args.text);
        self.notify_lobby(&lobby_id).await.ok();

        Ok(())
    }

    pub async fn play_card(&self, args: PlayCardArgs, user: &Claims) -> AppResult<()> {
        let lobby_id = args.code.clone();
        let lobby_arc = self.get_lobby(&lobby_id).await?;
        let (player_arc, card_arc) = {
            let lobby = lobby_arc.lock().await;
            let player = Arc::clone(&lobby.data.game_state.players.get(&user.sub).unwrap().player);
            let card =
                player.clone().lock().await.cards_in_hand[args.in_hand_index as usize].clone();
            (player, card)
        };
        let target = Self::convert(args.target, &lobby_arc).await;
        let game_arc = {
            let lobby = lobby_arc.lock().await;
            lobby.cloned_game().await
        };
        let lobby_manager_clone = self.clone();
        let lobby_id_clone = lobby_id.clone();

        {
            let mut game = game_arc.lock().await;
            game.play_card(&player_arc, args.in_hand_index as usize, target.clone())
                .await
                .map_err(|x| AppError::BadRequest(x))?;
        }

        let ga = Arc::clone(&game_arc);
        tokio::spawn(async move {
            Game::process_action_queue(ga, card_arc).await;
        });

        // Return immediately
        Ok(())
    }

    pub async fn update_game_state(&self, lobby_id: &str) {
        let hash_map = self.lobbies.lock().await;
        let mut lobby = hash_map.get(lobby_id).unwrap().lock().await;
        let game = lobby.cloned_game().await;
        match lobby.data.game_state.status {
            GameStatus::NeedsPlayers => {
                let all_ready = lobby
                    .data
                    .game_state
                    .players
                    .values()
                    .all(|player| player.status == PlayerStatus::Ready);

                if all_ready && lobby.data.game_state.players.len() > 1 {
                    lobby.data.game_state.status = GameStatus::WaitingForStart(5);
                }
            }
            GameStatus::WaitingForStart(duration) => {
                lobby.data.game_state.status = GameStatus::WaitingForStart(duration - 1);
                if lobby.data.game_state.status == GameStatus::WaitingForStart(1) {
                    lobby.data.game_state.status = GameStatus::InGame;
                    lobby.start_game().await;
                }
            }
            GameStatus::InGame => {
                let phase = {
                    lobby.data.game_state.public_info = lobby.get_state().await;
                    lobby
                        .data
                        .game_state
                        .public_info
                        .current_turn
                        .clone()
                        .unwrap()
                        .phase
                };

                for (_, player) in lobby.data.game_state.players.iter_mut() {
                    let mut hand = Vec::new();
                    let mut cards_in_play = Vec::new();
                    let mut spells = Vec::new();
                    let player_cards_in_play = &player.player.lock().await.cards_in_play.clone();
                    let player_spells = &player.player.lock().await.spells.clone();
                    let player_cards_in_hand = &player.player.lock().await.cards_in_hand.clone();

                    for card in player_cards_in_play {
                        cards_in_play.push(
                            CardWithDetails::from_card_arc(card, phase.clone(), true, &game).await,
                        );
                    }

                    for card in player_spells {
                        spells.push(
                            CardWithDetails::from_card_arc(card, phase.clone(), false, &game).await,
                        );
                    }

                    for card in player_cards_in_hand {
                        hand.push(
                            CardWithDetails::from_card_arc(card, phase.clone(), false, &game).await,
                        );
                    }

                    {
                        let game_player = player.player.lock().await;

                        player.public_info.spells = spells;
                        player.public_info.hand_size = hand.len() as i32;
                        player.public_info.cards_in_play = cards_in_play;
                        player.public_info.mana_pool = game_player.mana_pool.clone();
                        player.public_info.health = game_player
                            .stat_manager
                            .get_stat_value(crate::game::stat::StatType::Health);
                    }
                    player.hand = hand;
                }
            }
        }
    }

    pub async fn send_command(
        &self,
        lobby_id: &str,
        command: LobbyCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Step 1: Get the Redis connection
        let mut redis_conn = self.redis_client.get_multiplexed_async_connection().await?;

        let lobby_data = serde_json::to_string(&command)?;
        // Step 5: Publish the data to Redis.
        redis_conn.publish(lobby_id, lobby_data).await?;

        Ok(())
    }

    pub async fn notify_lobby(&self, lobby_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.update_game_state(lobby_id).await;

        // Step 1: Get the Redis connection
        let mut redis_conn = self.redis_client.get_multiplexed_async_connection().await?;

        // Step 2: Lock `lobbies` and extract the `lobby` reference.
        let lobby = {
            let lobbies = self.lobbies.lock().await;
            let lobby = lobbies.get(lobby_id).ok_or("Lobby not found")?.clone();
            lobby // release lobbies lock here
        };

        // Step 3: Now lock the `lobby` with a timeout to detect potential deadlock.
        let data = LobbyCommand::Updated({
            let lobby = match timeout(Duration::from_secs(5), lobby.lock()).await {
                Ok(lock) => lock.data.clone(),
                Err(_) => {
                    eprintln!("Timeout trying to acquire lobby lock");
                    return Err("Timeout while locking lobby".into());
                }
            };
            lobby
        });

        // Step 4: Serialize the lobby data.
        let lobby_data = serde_json::to_string(&data)?;

        // Step 5: Publish the data to Redis.
        redis_conn.publish(lobby_id, lobby_data).await?;

        Ok(())
    }

    pub async fn new(redis_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            redis_client: Arc::new(client),
            lobbies: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
