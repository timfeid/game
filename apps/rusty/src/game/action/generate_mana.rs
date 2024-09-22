use std::sync::Arc;

use tokio::sync::Mutex;

use crate::game::{card::Card, mana::ManaType, player::Player, turn::Turn, Game};

use super::{CardAction, PlayerAction, PlayerActionTarget};

#[derive(Debug, Clone)]
pub struct GenerateManaAction {
    pub mana_to_add: Vec<ManaType>,
    pub target: PlayerActionTarget,
}

#[async_trait::async_trait]
impl CardAction for GenerateManaAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        let owner = card.lock().await.owner.clone().unwrap();
        let player = &mut owner.lock().await;
        for mana in &self.mana_to_add {
            player.mana_pool.add_mana(*mana);
        }
    }
}
