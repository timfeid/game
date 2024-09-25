use std::sync::Arc;

use tokio::sync::Mutex;

use crate::game::stat::{StatType, Stats};

use super::{
    action::{Action, CardActionTarget},
    card::Card,
    effects::EffectTarget,
    player::Player,
    Game,
};

#[derive(Debug, Default)]
pub struct Combat {
    pub attackers: Vec<(Arc<Mutex<Card>>, EffectTarget)>,
    pub blockers: Vec<(Arc<Mutex<Card>>, Arc<Mutex<Card>>)>,
}

impl Combat {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn declare_attacker(&mut self, card: Arc<Mutex<Card>>, target: EffectTarget) {
        self.attackers.push((card, target));
    }

    pub async fn declare_blockers(&mut self, card: Arc<Mutex<Card>>, target: Arc<Mutex<Card>>) {
        self.blockers.push((card, target));
    }

    pub async fn resolve_combat(&mut self) {
        println!("Resolving combat damage.");

        // First, resolve blocked attackers
        for (blocking_card, attacking_card) in &self.blockers {
            // let attacker_card_arc = match target {
            //     EffectTarget::Card(card_arc) => Arc::clone(card_arc),
            //     _ => continue, // Skip if the target is not a card
            // };
            let attacker_card_arc = attacking_card;

            let blocker_card_arc = blocking_card;

            // Get stats
            let attacker_damage = {
                let attacker_card = attacker_card_arc.lock().await;
                attacker_card.get_stat_value(StatType::Damage)
            };

            let blocker_damage = {
                let blocker_card = blocker_card_arc.lock().await;
                blocker_card.get_stat_value(StatType::Damage)
            };

            // Apply damage to blocker
            {
                let mut blocker_card = blocker_card_arc.lock().await;
                blocker_card.modify_stat(StatType::Defense, -attacker_damage);
                println!(
                    "Attacker {} deals {} damage to blocker {}",
                    attacker_card_arc.lock().await.name,
                    attacker_damage,
                    blocker_card.name
                );
            }

            // Apply damage to attacker
            {
                let mut attacker_card = attacker_card_arc.lock().await;
                attacker_card.modify_stat(StatType::Defense, -blocker_damage);
                println!(
                    "Blocker {} deals {} damage to attacker {}",
                    blocker_card_arc.lock().await.name,
                    blocker_damage,
                    attacker_card.name
                );
            }
        }

        // Then, resolve unblocked attackers
        for (attacker_card_arc, target) in &self.attackers {
            // Check if this attacker was blocked
            let mut is_blocked = false;

            for (_, attacker) in &self.blockers {
                if Arc::ptr_eq(attacker_card_arc, attacker) {
                    is_blocked = true;
                }
            }

            if !is_blocked {
                let attacker_damage = {
                    let attacker_card = attacker_card_arc.lock().await;
                    attacker_card.get_stat_value(StatType::Damage)
                };

                // Apply damage to the target
                match target {
                    EffectTarget::Player(player_arc) => {
                        let mut player = player_arc.lock().await;
                        player.modify_stat(StatType::Health, -attacker_damage);
                        println!(
                            "Attacker {} deals {} damage to player {}",
                            attacker_card_arc.lock().await.name,
                            attacker_damage,
                            player.name
                        );
                    }
                    EffectTarget::Card(card_arc) => {
                        let mut card = card_arc.lock().await;
                        card.modify_stat(StatType::Defense, -attacker_damage);
                        println!(
                            "Attacker {} deals {} damage to card {}",
                            attacker_card_arc.lock().await.name,
                            attacker_damage,
                            card.name
                        );
                    }
                }
            }
        }
        self.attackers = vec![];
        self.blockers = vec![];
    }
}
