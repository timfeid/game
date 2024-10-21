use std::{any::Any, future::Future, pin::Pin, sync::Arc};

use tokio::sync::Mutex;

use crate::game::{
    card::Card,
    effects::{AddTriggerEffect, Effect, EffectID, EffectTarget, ExpireContract},
    mana::ManaType,
    player::Player,
    turn::Turn,
    Game,
};

use super::{
    ActionTriggerType, AsyncClosureAction, CardAction, CardActionTrigger, CardRequiredTarget,
    PlayerAction, PlayerActionTarget,
};

#[derive(Debug, Clone)]
pub struct GenerateManaAction {
    pub mana_to_add: Vec<ManaType>,
    pub target: PlayerActionTarget,
}

fn untap_and_remove() -> CardActionTrigger {
    CardActionTrigger::new(
        ActionTriggerType::AbilityWithinPhases("Undo tap".to_string(), vec![], None, false),
        CardRequiredTarget::None,
        Arc::new(AsyncClosureAction::new(Arc::new(
            |game: Arc<Mutex<Game>>,
             card: Arc<Mutex<Card>>|
             -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                Box::pin({
                    async move {
                        let mana: Vec<GenerateManaAction> = card
                            .lock()
                            .await
                            .triggers
                            .iter()
                            .filter(|t| {
                                t.action
                                    .as_any()
                                    .downcast_ref::<GenerateManaAction>()
                                    .is_some()
                            })
                            .map(|x| {
                                x.action
                                    .as_any()
                                    .downcast_ref::<GenerateManaAction>()
                                    .unwrap()
                                    .clone()
                            })
                            .collect();
                        let trigger_index = card.lock().await.triggers.len() - 1;
                        card.lock().await.triggers.remove(trigger_index);
                        if mana.len() == 1 {
                            let mana = &mana[0].mana_to_add;

                            card.lock()
                                .await
                                .owner
                                .as_ref()
                                .unwrap()
                                .lock()
                                .await
                                .pay_mana(mana)
                                .await?;
                            card.lock().await.untap();
                        }
                        Ok(())
                    }
                })
            },
        ))),
    )
}

#[async_trait::async_trait]
impl CardAction for GenerateManaAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner = card.lock().await.owner.clone().unwrap();
        if let EffectTarget::Card(card) = target {
            let game_arc = Arc::new(Mutex::new(std::mem::take(game)));
            let trigger = Arc::new(Mutex::new(AddTriggerEffect::new(
                card.clone(),
                ExpireContract::Steps(1),
                Some(card.clone()),
                untap_and_remove(),
                game_arc.clone(),
            )));
            let mut game_unlocked = game_arc.lock().await;
            *game = std::mem::take(&mut *game_unlocked);

            game.effect_manager.add_effect(
                EffectID(format!(
                    "{}-{}",
                    card.lock().await.id,
                    ability_id.unwrap_or("unkonwn".to_string())
                )),
                trigger,
            );
        }
        let player = &mut owner.lock().await;
        for mana in &self.mana_to_add {
            player.mana_pool.add_mana(*mana);
        }
        Ok(())
    }
}
