use super::{
    action::{Action, CardActionTarget},
    card::Card,
    effects::EffectTarget,
    player::Player,
    Game,
};
use crate::game::stat::{Stat, StatType, Stats};
use std::sync::Arc;
use tokio::sync::Mutex;
use ulid::Ulid;

#[derive(Debug, Default)]
pub struct Combat {
    pub attackers: Vec<(Arc<Mutex<Card>>, EffectTarget)>, // Attacking creatures and their targets
    pub blockers: Vec<(Arc<Mutex<Card>>, Arc<Mutex<Card>>)>, // Blockers and the creatures they are blocking
}

impl Combat {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare an attacker
    pub async fn declare_attacker(&mut self, card: Arc<Mutex<Card>>, target: EffectTarget) {
        self.attackers.push((card, target));
    }

    /// Declare a blocker
    pub async fn declare_blocker(
        &mut self,
        blocker_card: Arc<Mutex<Card>>,
        attacker_card: Arc<Mutex<Card>>,
    ) {
        self.blockers.push((blocker_card, attacker_card));
    }

    pub async fn resolve_combat(&mut self) -> Vec<Arc<Mutex<Card>>> {
        println!("Resolving combat damage.");
        let mut destroyed_cards = Vec::new();

        // First, resolve damage to blockers
        for (blocking_card_arc, attacker_card_arc) in &self.blockers {
            let attacker_damage = {
                let attacker_card = attacker_card_arc.lock().await;
                attacker_card.get_stat_value(StatType::Power)
            };

            let blocker_toughness = {
                let blocker_card = blocking_card_arc.lock().await;
                blocker_card.get_stat_value(StatType::Toughness)
            };

            // Apply damage to the blocker
            {
                let mut blocker_card = blocking_card_arc.lock().await;
                blocker_card.damage_taken += attacker_damage;
                println!(
                    "Attacker {} deals {} damage to blocker {}",
                    attacker_card_arc.lock().await.name,
                    attacker_damage,
                    blocker_card.name
                );

                // Check if blocker is destroyed
                if blocker_card.damage_taken >= blocker_toughness {
                    println!("Blocker {} is destroyed!", blocker_card.name);
                    destroyed_cards.push(Arc::clone(&blocking_card_arc));
                }
            }

            // Apply damage to the attacker (from the blocker)
            {
                let blocker_damage = {
                    let blocker_card = blocking_card_arc.lock().await;
                    blocker_card.get_stat_value(StatType::Power)
                };
                let attacker_toughness = {
                    let attacker_card = attacker_card_arc.lock().await;
                    attacker_card.get_stat_value(StatType::Toughness)
                };
                let mut attacker_card = attacker_card_arc.lock().await;
                attacker_card.damage_taken += blocker_damage;
                println!(
                    "Blocker {} deals {} damage to attacker {}",
                    blocking_card_arc.lock().await.name,
                    blocker_damage,
                    attacker_card.name
                );

                // Check if attacker is destroyed
                if attacker_card.damage_taken >= attacker_toughness {
                    println!("Attacker {} is destroyed!", attacker_card.name);
                    destroyed_cards.push(Arc::clone(attacker_card_arc));
                }
            }
        }

        // Then, resolve unblocked attackers and handle Trample
        for (attacker_card_arc, target) in &self.attackers {
            // Check if this attacker was blocked
            let mut is_blocked = false;
            let mut total_blocker_toughness = 0;

            for (blocking_card_arc, blocked_attacker_card_arc) in &self.blockers {
                if Arc::ptr_eq(attacker_card_arc, blocked_attacker_card_arc) {
                    is_blocked = true;
                    let blocker_toughness = {
                        let blocker_card = blocking_card_arc.lock().await;
                        blocker_card.get_stat_value(StatType::Toughness)
                    };
                    total_blocker_toughness += blocker_toughness;
                }
            }

            let attacker_damage = {
                let attacker_card = attacker_card_arc.lock().await;
                attacker_card.get_stat_value(StatType::Power)
            };

            // Handle unblocked attackers
            if !is_blocked {
                self.apply_damage_to_target(attacker_damage, target, attacker_card_arc)
                    .await;
            } else {
                // Handle blocked attackers and check for Trample
                let has_trample = {
                    let attacker_card = attacker_card_arc.lock().await;
                    attacker_card.get_stat_value(StatType::Trample) > 0
                };

                if has_trample {
                    let excess_damage = attacker_damage.saturating_sub(total_blocker_toughness);
                    if excess_damage > 0 {
                        self.apply_damage_to_target(excess_damage, target, attacker_card_arc)
                            .await;
                    }
                }
            }
        }

        self.attackers.clear();
        self.blockers.clear();

        destroyed_cards
    }

    async fn apply_damage_to_target(
        &self,
        damage: i8,
        target: &EffectTarget,
        attacker_card_arc: &Arc<Mutex<Card>>,
    ) -> Option<Arc<Mutex<Card>>> {
        match target {
            EffectTarget::Player(player_arc) => {
                let mut player = player_arc.lock().await;
                let id = format!(
                    "damage-{}-{}",
                    attacker_card_arc.lock().await.name,
                    // self.attackers.first().unwrap().0.lock().await.name
                    Ulid::new()
                );
                player.add_stat(id, Stat::new(StatType::Health, -damage));
                {
                    attacker_card_arc.lock().await.damage_dealt_to_players = damage.clone();
                }
                println!(
                    "Attacker {} deals {} damage to player {}",
                    attacker_card_arc.lock().await.name,
                    damage,
                    player.name
                );

                None
            }
            EffectTarget::Card(card_arc) => {
                let mut card = card_arc.lock().await;
                let toughness = card.get_stat_value(StatType::Toughness);
                card.damage_taken += damage;
                println!(
                    "Attacker {} deals {} damage to card {}",
                    attacker_card_arc.lock().await.name,
                    damage,
                    card.name
                );

                if card.damage_taken >= toughness {
                    println!("Card {} is destroyed!", card.name);
                    Some(Arc::clone(card_arc))
                } else {
                    None
                }
            }
        }
    }
}
