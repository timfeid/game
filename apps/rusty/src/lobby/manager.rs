use futures::stream::StreamExt;
use futures::Stream;
use redis::aio::PubSub;
use redis::{AsyncCommands, Client};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::lobby::{Lobby, LobbyData};
use crate::error::{AppError, AppResult};
use crate::game::Game;
use crate::services::jwt::{Claims, JwtService};

#[derive(Clone)]
pub struct LobbyManager {
    redis_client: Arc<redis::Client>,
    lobbies: Arc<Mutex<HashMap<String, Arc<Mutex<Lobby>>>>>,
}

impl std::fmt::Debug for LobbyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LobbyManager")
            .field("lobbies", &self.lobbies)
            .finish()
    }
}

impl LobbyManager {
    pub async fn create_lobby(&self, user: &Claims) -> AppResult<String> {
        let mut lobbies = self.lobbies.lock().await;
        let lobby = Lobby::new(user);
        let lobby_id = lobby.data.join_code.clone();
        lobbies.insert(lobby_id.clone(), Arc::new(Mutex::new(lobby)));

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
    ) -> AppResult<impl Stream<Item = LobbyData>> {
        let user = JwtService::decode(&access_token).or(Err(AppError::Unauthorized))?;
        let (tx, rx) = mpsc::channel::<LobbyData>(100);

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
        tx: mpsc::Sender<LobbyData>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut pubsub_conn = redis_client.get_async_pubsub().await?;
        pubsub_conn.subscribe(&lobby_id).await?;

        let mut pubsub_stream = pubsub_conn.on_message();
        while let Some(message) = pubsub_stream.next().await {
            let payload: String = message.get_payload()?;
            if let Ok(game) = serde_json::from_str::<LobbyData>(&payload) {
                if tx.send(game).await.is_err() {
                    eprintln!("Receiver dropped");
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn notify_lobby(&self, lobby_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("updating the lobby");
        let mut redis_conn = self.redis_client.get_multiplexed_async_connection().await?;

        // Step 1: Lock `lobbies` and extract the `lobby` reference.
        let lobby = {
            let lobbies = self.lobbies.lock().await;
            lobbies.get(lobby_id).ok_or("Lobby not found")?.clone()
        };
        println!("updating the lobby2");
        let data = lobby.lock().await.data.clone();
        println!("updating the lobby3");

        // Step 2: Lock the specific `lobby` and extract its data after releasing the `lobbies` lock.
        let lobby_data = serde_json::to_string(&data)?;

        // Step 3: Publish the data to Redis.
        redis_conn.publish(lobby_id, lobby_data).await?;
        println!("Updated the lobby!");
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
