use std::{any::Any, sync::Arc};

use tokio::sync::Mutex;

use crate::game::{
    card::Card,
    effects::EffectTarget,
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
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        println!("add stat? target: {:?}", target);
        let mut card = card.lock().await;
        card.add_stat(self.id.clone(), self.stat.clone());
    }
}
