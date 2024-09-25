use std::sync::Arc;

use tokio::sync::Mutex;

use crate::game::{
    card::Card,
    stat::{Stat, StatType, Stats},
    Game,
};

use super::CardAction;

#[derive(Debug, Clone)]
pub struct CardAddStatAction {
    pub stat: Stat,
    pub id: String,
}

#[async_trait::async_trait]
impl CardAction for CardAddStatAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        println!("add stat?");
        let mut card = card.lock().await;
        card.add_stat(self.id.clone(), self.stat.clone());
    }
}
