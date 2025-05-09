use std::{any::Any, sync::Arc};

use tokio::sync::Mutex;

use crate::game::{
    card::Card,
    effects::EffectTarget,
    player::Player,
    stat::{Stat, StatType, Stats},
    FrontendTarget, Game,
};

use super::CardAction;

#[derive(Debug, Clone)]
pub struct CardAddStatAction {
    pub stat: Stat,
    pub id: String,
}

#[async_trait::async_trait]
impl CardAction for CardAddStatAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        println!("add stat? target: {:?}", target);
        let mut card = card.lock().await;
        card.add_stat(self.id.clone(), self.stat.clone());
        Ok(())
    }
}
