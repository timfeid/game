use std::{cell::RefCell, rc::Rc, sync::Arc};

use tokio::sync::Mutex;

use crate::game::{
    action::{Action, CardAction},
    card::Card,
    player::Player,
    stat::{StatType, Stats},
    turn::{Turn, TurnPhase},
    Game,
};

use super::EffectTarget;

#[derive(Debug, Clone, PartialEq)]
pub enum CardActionTarget {
    This,   // The card itself
    Owner,  // The owner of the card
    Target, // The card's target (like a player or another card)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModifyStatAction {
    pub stat: StatType,
    pub amount: i8,
    pub target: CardActionTarget,
    pub phases: Option<Vec<TurnPhase>>, // Optional phases: None means the action can be fired manually
}

#[async_trait::async_trait]
impl CardAction for ModifyStatAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        match self.target {
            CardActionTarget::This => {
                // Lock the card to get a mutable reference and modify it
                let mut card = card.lock().await;
                card.modify_stat(self.stat, self.amount);
            }
            CardActionTarget::Owner => {
                // Lock the owner to modify the owner's stats
                if let Some(owner) = &card.lock().await.owner {
                    let mut owner = owner.lock().await;
                    owner.modify_stat(self.stat, self.amount);
                } else {
                    println!(
                        "no owner of card {}",
                        card.lock().await.render(30).join("\n")
                    );
                }
                // let mut owner = card.lock().await.owner.and_then(|f| f)owner.lock().await;
            }
            CardActionTarget::Target => {
                // Apply the stat modification to the target of the card
                if let Some(target) = &card.lock().await.target {
                    match target {
                        EffectTarget::Player(target_stats) => {
                            let mut stats = target_stats.lock().await;
                            stats.modify_stat(self.stat, self.amount);
                        }
                        EffectTarget::Card(target_stats) => {
                            let mut stats = target_stats.lock().await;
                            stats.modify_stat(self.stat, self.amount);
                        }
                    }
                }
            }
        }
    }

    // async fn applies_in_phase(&self, phase: Turn, _owner: Arc<Mutex<Player>>) -> bool {
    //     if let Some(phases) = &self.phases {
    //         phases.contains(&phase.phase)
    //     } else {
    //         true
    //     }
    // }
}
