use std::collections::HashMap;

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
    game: Game,
}

use redis::Client;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio_stream::wrappers::ReceiverStream;
use ulid::Ulid;

use crate::{
    game::{player::Player, Game, GameState},
    services::jwt::Claims,
};

impl Lobby {
    pub fn new(user: &Claims) -> Self {
        let mut lobby = Lobby {
            data: LobbyData::default(),
            client: None,
            game: Game::new(),
        };

        let player = Player::new(&user.sub.clone(), 100, 8, vec![]);

        lobby.game.add_player(player);

        lobby
    }

    pub fn join(&mut self, user: &Claims) -> &mut Self {
        self.game.add_player(Player::from_claims(user));
        self
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
        let lobby = &Rc::new(RefCell::new(Lobby::new(&user_id)));
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
            .message(&user_id2, "test".to_string());
    }
}
