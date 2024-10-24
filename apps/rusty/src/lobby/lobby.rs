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
pub struct DeckDetails {
    cards: Vec<CardWithCount>,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct CardWithCount {
    card: CardWithDetails,
    count: i32,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct LobbyData {
    pub join_code: String,
    pub chat: Vec<LobbyChat>,
    pub game_state: GameState,
}
impl Default for LobbyData {
    fn default() -> LobbyData {
        let mut game_state = GameState::default();
        let code = ulid::Ulid::new().to_string();
        game_state.code = code.clone();

        LobbyData {
            join_code: code,
            chat: vec![],
            game_state: game_state,
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
            let game = self.game.lock().await;
            if let Some((player, time_left, _)) = &game.current_priority_player {
                Some(PriorityQueue {
                    player_id: player.lock().await.name.clone(),
                    time_left: time_left.clone(),
                })
            } else {
                None
            }
        };

        let combat = self.game.lock().await.combat.clone();
        let blocks = {
            let mut blocks = vec![];
            for (blocker, attacker) in combat.blockers.iter() {
                blocks.push(Block {
                    attacker: {
                        Game::frontend_target_from_card(&self.game, attacker)
                            .await
                            .expect("hm")
                    },
                    blocker: {
                        Game::frontend_target_from_card(&self.game, blocker)
                            .await
                            .expect("hm")
                    },
                })
            }

            blocks
        };

        let attacks = {
            let mut attacks = vec![];
            let cloned_game = self.cloned_game();
            let game = cloned_game.lock().await;
            if let Some(turn) = game.current_turn.clone() {
                let player = &turn.current_player;
                let player_id = turn.current_player.lock().await.name.clone();
                let cards = player.lock().await.cards_in_play.clone();
                for (index, card) in cards.iter().enumerate() {
                    for (attacker, target) in game.combat.attackers.iter() {
                        if Arc::ptr_eq(attacker, card) {
                            attacks.push(Attack {
                                target: target.clone(),
                                attacker: FrontendCardTarget {
                                    player_id: player_id.clone(),
                                    pile: FrontendPileName::Play,
                                    card_index: index as i32,
                                },
                            });
                        }
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

    pub fn cloned_game(&self) -> Arc<Mutex<Game>> {
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
    // Elves,
    // Elves2,
    // Blue,
    // Black,
    Angels,
    // AngelsBlue,
    // Red,
}

use crate::{
    error::{AppError, AppResult},
    game::{
        card::Card, decks::Deck, player::Player, Attack, Block, CardWithDetails,
        FrontendCardTarget, FrontendPileName, FrontendTarget, Game, GameState, GameStatus,
        PlayerState, PlayerStatus, PriorityQueue, PublicGameInfo,
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
                let player = Player::from_claims(user, game.starting_health);
                let player = game.add_player(player).await;
                let index = game.players.len() - 1;
                (index, player)
            };

            let player = self.data.game_state.players.insert(
                user.sub.clone(),
                PlayerState::from_player(player, index as i32, user.sub.clone()),
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

    pub async fn list_desks(&mut self, user: &Claims) -> Vec<DeckSelector> {
        return vec![
            // DeckSelector::Elves,
            // DeckSelector::Elves2,
            DeckSelector::Angels,
            // DeckSelector::Black,
            // DeckSelector::Blue,
            // DeckSelector::AngelsBlue,
        ];
    }

    pub async fn get_deck_info(&self, deck: DeckSelector) -> DeckDetails {
        let cards = Deck::cards_from_selection(&deck);
        let mut card_map: HashMap<String, (CardWithDetails, usize)> = HashMap::new();

        // Group the cards by name
        for card in cards {
            let card_details = CardWithDetails::from_card(card).await;
            let card_name = card_details.card.name.clone(); // Assuming CardWithDetails has a `name` field

            card_map
                .entry(card_name)
                .and_modify(|(_, count)| *count += 1)
                .or_insert((card_details, 1));
        }

        // Convert the HashMap into a Vec of CardWithCount
        let mut cards_with_count: Vec<CardWithCount> = card_map
            .into_iter()
            .map(|(_, (card, count))| CardWithCount {
                card,
                count: count as i32,
            })
            .collect();

        // Sort by card count (descending) and name (ascending)
        cards_with_count.sort_by(|a, b| {
            // First compare by count (descending)
            b.count
                .cmp(&a.count)
                // Then compare by name (ascending) if counts are the same
                .then_with(|| a.card.card.name.cmp(&b.card.card.name))
        });

        DeckDetails {
            cards: cards_with_count,
        }
    }

    pub async fn ready(&mut self, user: &Claims) -> &mut Self {
        if let Some(player) = self.data.game_state.players.get_mut(&user.sub) {
            player.status = PlayerStatus::Ready;
            let mut p = player.player.lock().await;
            let mut deck = Deck::new_from_selection(&player.deck);
            deck.set_owner(&player.player).await;

            p.deck = deck;
        }

        self
    }

    pub async fn respond_card_selection(
        &mut self,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
    ) -> AppResult<()> {
        Game::respond_card_selection(&self.game, &player, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn respond_mandatory_player_ability(
        &mut self,
        ability_id: String,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
    ) -> AppResult<()> {
        Game::respond_player_ability(&self.game, &player, ability_id, true, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn respond_optional_player_ability(
        &mut self,
        ability_id: String,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        response: bool,
    ) -> AppResult<()> {
        Game::respond_player_ability(&self.game, &player, ability_id, response, target)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn action_card(
        &mut self,
        frontend_card: FrontendCardTarget,
        target: Option<FrontendTarget>,
        trigger_id: String,
    ) -> AppResult<()> {
        // let current_player = Arc::clone(&self.game.current_turn.as_ref().unwrap().current_player);
        Game::activate_card_action(&self.game, frontend_card, target, trigger_id)
            .await
            .map_err(|x| AppError::BadRequest(x))?;

        Ok(())
    }

    pub async fn advance_turn(&mut self) {
        let game = self.game.clone();
        tokio::spawn(async move {
            Game::advance_turn(&game).await;
        });
    }

    pub async fn start_game(&mut self) {
        Game::start(&self.game).await;
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
