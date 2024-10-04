use std::{borrow::BorrowMut, collections::HashMap, sync::Arc, thread::Thread};

use futures::StreamExt;

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct LobbyChat {
    user_id: String,
    message: String,
}
impl LobbyChat {
    pub fn new(user_id: String, message: String) -> Self {
        Self { user_id, message }
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct LobbyData {
    pub join_code: String,
    pub chat: Vec<LobbyChat>,
    pub game_state: GameState,
}
impl Default for LobbyData {
    fn default() -> LobbyData {
        LobbyData {
            join_code: ulid::Ulid::new().to_string(),
            chat: vec![],
            game_state: GameState::default(),
        }
    }
}

#[derive(Type, Deserialize, Serialize, Debug)]
pub struct Lobby {
    #[serde(skip_serializing, skip_deserializing)]
    client: Option<Client>,

    pub data: LobbyData,

    #[serde(skip_serializing, skip_deserializing)]
    game: Arc<RwLock<Game>>,
}

impl Lobby {
    pub async fn get_state(&self) -> PublicGameInfo {
        PublicGameInfo {
            current_turn: self.game.read().await.current_turn.clone(),
        }
    }

    pub async fn cloned_game(&self) -> Arc<RwLock<Game>> {
        Arc::clone(&self.game)
    }
}

use redis::Client;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{Mutex, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use ulid::Ulid;

use crate::{
    error::{AppError, AppResult},
    game::{
        deck::Deck, effects::EffectTarget, player::Player, Game, GameState, GameStatus,
        PlayerState, PlayerStatus, PublicGameInfo,
    },
    services::jwt::Claims,
};

use super::manager::LobbyManager;

impl Lobby {
    pub async fn new(user: &Claims) -> Self {
        let game = Game::new();
        let mut lobby = Lobby {
            data: LobbyData::default(),
            client: None,
            game: Arc::new(RwLock::new(game)),
        };

        let player = Player::new(&user.sub.clone(), 20, 8, vec![]);

        lobby.join(user).await;

        lobby
    }

    pub async fn join(&mut self, user: &Claims) -> &mut Self {
        if !self.data.game_state.players.contains_key(&user.sub) {
            let (index, player) = {
                let mut game = self.game.write().await;
                let player = Player::from_claims(user);
                let player = game.add_player(player).await;
                let index = game.players.len() - 1;
                (index, player)
            };

            let player = self.data.game_state.players.insert(
                user.sub.clone(),
                PlayerState::from_player(player, index as i32),
            );
            if self.data.game_state.players.len() == 1 {
                self.data
                    .game_state
                    .players
                    .get_mut(&user.sub)
                    .unwrap()
                    .is_leader = true;
                println!("setting leader");
            }
        }

        // println!("JOIN {:?}", self);

        self
    }

    pub async fn ready(&mut self, user: &Claims) -> &mut Self {
        if let Some(player) = self.data.game_state.players.get_mut(&user.sub) {
            player.status = PlayerStatus::Ready;
            let mut p = player.player.lock().await;
            let deck = Deck::new(Deck::create_green_deck());
            deck.set_owner(&player.player).await;

            p.deck = deck;
            p.deck.shuffle();
        }

        self
    }

    pub async fn attach_card(
        &mut self,
        player_index: usize,
        in_play_index: usize,
        target: Option<EffectTarget>,
    ) -> AppResult<()> {
        // let current_player = Arc::clone(&self.game.current_turn.as_ref().unwrap().current_player);
        let player = Arc::clone(&self.game.read().await.players[player_index]);

        self.game
            .write()
            .await
            .attach_card_action(&player, in_play_index, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn action_card(
        &mut self,
        player_index: usize,
        in_play_index: usize,
        target: Option<EffectTarget>,
    ) -> AppResult<()> {
        // let current_player = Arc::clone(&self.game.current_turn.as_ref().unwrap().current_player);
        let player = Arc::clone(&self.game.read().await.players[player_index]);

        self.game
            .write()
            .await
            .activate_card_action(&player, in_play_index, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn play_card(
        &mut self,
        player: Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> AppResult<()> {
        self.game
            .write()
            .await
            .play_card(&player, index, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn advance_turn(&mut self) {
        self.game.write().await.advance_turn().await;
    }

    pub async fn start_game(&mut self) {
        self.game.write().await.start().await;
    }

    pub fn message(&mut self, user: &Claims, message: String) -> &mut Self {
        self.data
            .chat
            .push(LobbyChat::new(user.sub.clone(), message));

        self
    }
}

mod test {
    use std::{cell::RefCell, rc::Rc};

    use tokio_stream::StreamExt;

    use crate::{lobby::lobby::Lobby, services::jwt::Claims};

    #[tokio::test]
    async fn test() {
        let user_id = Claims {
            sub: "boob".to_string(),
            jti: Some("boob".to_string()),
            exp: 0,
        };
        let user_id2 = Claims {
            sub: "sakdfakjs".to_string(),
            jti: Some("asdkjfjskd".to_string()),
            exp: 0,
        };
        let lobby = &Rc::new(RefCell::new(Lobby::new(&user_id).await));
        let redis_url = "redis://127.0.0.1/".to_string();
        let redis = redis::Client::open(redis_url).unwrap();

        // async_stream::stream! {
        //     // let mut post_stream = lobby.clone().borrow_mut().subscribe(redis);
        //     while let Some(post) = post_stream.next().await {
        //         println!("{:?}", post);
        //         yield post;
        //     }
        // };

        lobby
            .clone()
            .borrow_mut()
            .join(&user_id2)
            .await
            .message(&user_id2, "test".to_string());
    }
}
