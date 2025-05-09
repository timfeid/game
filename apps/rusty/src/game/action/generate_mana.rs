use std::{any::Any, future::Future, pin::Pin, sync::Arc};

use tokio::sync::Mutex;

use crate::game::{
    card::Card,
    effects::{AddTriggerEffect, Effect, EffectID, EffectTarget, ExpireContract},
    mana::ManaType,
    player::Player,
    turn::Turn,
    FrontendTarget, Game,
};

use super::{
    ActionBuilder, ActionTriggerType, AsyncClosureAction, CardAction, CardActionTrigger,
    CardRequiredTarget, PlayerAction, PlayerActionTarget,
};

#[derive(Debug, Clone)]
pub struct GenerateManaAction {
    pub mana_to_add: Vec<ManaType>,
    pub target: PlayerActionTarget,
}

fn untap_and_remove() -> CardActionTrigger {
    ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
        "Undo tap".to_string(),
        vec![],
        None,
        false,
        CardRequiredTarget::None,
    ))
    .closure_action(|game, card, owner, target, ability| {
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

                    owner.lock().await.pay_mana(mana)?;
                    card.lock().await.untap();
                }
                Ok(())
            }
        })
    })
    .build()
}

#[async_trait::async_trait]
impl CardAction for GenerateManaAction {
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
        if let Some(FrontendTarget::Card(card)) = &target {
            let card = Game::card_from_frontend_card_target(&game, card).await;
            let trigger = Arc::new(Mutex::new(AddTriggerEffect::new(
                card.clone(),
                ExpireContract::Steps(1),
                Some(card.clone()),
                untap_and_remove(),
                game.clone(),
            )));

            game.lock().await.effect_manager.add_effect(
                EffectID(format!(
                    "{}-{}",
                    card.lock().await.id,
                    ability_id.unwrap_or("unkonwn".to_string())
                )),
                trigger,
            );
        }
        for mana in &self.mana_to_add {
            player.lock().await.mana_pool.add_mana(*mana);
        }
        Ok(())
    }
}
