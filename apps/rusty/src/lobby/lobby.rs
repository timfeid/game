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
    game: Arc<Mutex<Game>>,
}

impl Lobby {
    pub async fn get_state(&self) -> PublicGameInfo {
        let priority_queue = {
            let cloned_game = self.cloned_game().await;
            let game = cloned_game.lock().await;
            if let Some((player, time_left, _)) = &game.current_priority_player {
                Some(PriorityQueue {
                    player_index: self
                        .data
                        .game_state
                        .players
                        .values()
                        .find(|x| Arc::ptr_eq(&x.player, player))
                        .unwrap()
                        .player_index,
                    time_left: time_left.clone(),
                })
            } else {
                None
            }
        };

        let blocks = {
            let mut blocks = vec![];
            let cloned_game = self.cloned_game().await;
            let game = cloned_game.lock().await;
            for (blocker, attacker) in game.combat.blockers.iter() {
                blocks.push(Block {
                    attacker: { game.frontend_target_from_card(attacker).await },
                    blocker: { game.frontend_target_from_card(blocker).await },
                })
            }

            blocks
        };

        let attacks = {
            let mut attacks = vec![];
            let cloned_game = self.cloned_game().await;
            let game = cloned_game.lock().await;
            let turn = game.current_turn.clone().unwrap();
            let player = turn.current_player;
            for (index, card) in player.lock().await.cards_in_play.iter().enumerate() {
                for (attacker, target) in game.combat.attackers.iter() {
                    if Arc::ptr_eq(attacker, card) {
                        attacks.push(Attack {
                            target: game.frontend_target_from_effect_target(target).await,
                            attacker: FrontendCardTarget {
                                player_index: turn.current_player_index,
                                pile: FrontendPileName::Play,
                                card_index: index as i32,
                            },
                        });
                    }
                }
            }

            attacks
        };

        PublicGameInfo {
            current_turn: self.game.lock().await.current_turn.clone(),
            priority_queue,
            attacks,
            blocks,
        }
    }

    pub async fn cloned_game(&self) -> Arc<Mutex<Game>> {
        Arc::clone(&self.game)
    }
}

use redis::Client;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{Mutex, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use ulid::Ulid;

#[derive(Type, Deserialize, Clone, Serialize, Debug)]
pub enum DeckSelector {
    Green,
    Blue,
    Black,
    Angels,
}

use crate::{
    error::{AppError, AppResult},
    game::{
        decks::{angels::create_angels_deck, Deck},
        effects::EffectTarget,
        player::Player,
        Attack, Block, CardWithDetails, FrontendCardTarget, FrontendPileName, FrontendTarget, Game,
        GameState, GameStatus, PlayerState, PlayerStatus, PriorityQueue, PublicGameInfo,
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
            game: Arc::new(Mutex::new(game)),
        };

        let player = Player::new(&user.sub.clone(), 20, vec![]);

        lobby.join(user).await;

        lobby
    }

    pub async fn join(&mut self, user: &Claims) -> &mut Self {
        if !self.data.game_state.players.contains_key(&user.sub) {
            let (index, player) = {
                let mut game = self.game.lock().await;
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

    pub async fn select_deck(&mut self, user: &Claims, deck: DeckSelector) -> &mut Self {
        if let Some(player) = self.data.game_state.players.get_mut(&user.sub) {
            player.deck = deck;
        }

        self
    }

    pub async fn ready(&mut self, user: &Claims) -> &mut Self {
        if let Some(player) = self.data.game_state.players.get_mut(&user.sub) {
            player.status = PlayerStatus::Ready;
            let mut p = player.player.lock().await;
            let deck = Deck::new(match player.deck {
                DeckSelector::Green => Deck::create_green_deck(),
                DeckSelector::Blue => Deck::create_blue_deck(),
                DeckSelector::Black => Deck::create_black_deck(),
                DeckSelector::Angels => create_angels_deck(),
            });
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
        let player = Arc::clone(&self.game.lock().await.players[player_index]);

        self.game
            .lock()
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
        let player = Arc::clone(&self.game.lock().await.players[player_index]);

        self.game
            .lock()
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
            .lock()
            .await
            .play_card(&player, index, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn advance_turn(&mut self) {
        self.game.lock().await.advance_turn().await;
    }

    pub async fn start_game(&mut self) {
        self.game.lock().await.start().await;
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
