use fmt::Debug;
use std::{any::Any, collections::HashMap, fmt, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    action::CardAction,
    card::Card,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    Game,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ModifyStatTarget {
    Card,
    Owner,
    Target,
}

#[derive(Debug, Clone)]
pub enum EffectTarget {
    Player(Arc<Mutex<Player>>),
    Card(Arc<Mutex<Card>>),
}

// Define a unique identifier for each effect
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectID(Uuid);

impl EffectID {
    pub fn new() -> Self {
        EffectID(Uuid::new_v4())
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

// Trait for defining different types of effects
#[async_trait::async_trait]
pub trait Effect: Send + Sync + Debug {
    async fn apply(&mut self);
    fn is_expired(&self) -> bool;
    async fn cleanup(&mut self);
    fn get_id(&self) -> &EffectID;
    fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>>;
}

// Centralized `EffectManager` that manages effects via unique IDs
#[derive(Clone, Default)]
pub struct EffectManager {
    pub effects: HashMap<EffectID, Arc<Mutex<dyn Effect + Send + Sync>>>, // Use EffectID as the key
}

impl EffectManager {
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
        }
    }

    // Add a new effect with its ID
    pub fn add_effect(
        &mut self,
        effect_id: EffectID,
        effect: Arc<Mutex<dyn Effect + Send + Sync>>,
    ) {
        self.effects.insert(effect_id, effect);
    }

    // Remove an effect by its ID
    pub fn remove_effect(&mut self, effect_id: &EffectID) {
        self.effects.remove(effect_id);
    }

    pub async fn apply_effects(&mut self) {
        let effect_ids: Vec<EffectID> = self.effects.keys().cloned().collect();
        for effect_id in effect_ids {
            if let Some(effect_arc) = self.effects.clone().get(&effect_id) {
                let mut effect = effect_arc.lock().await;
                effect.apply().await;
                if effect.is_expired() {
                    effect.cleanup().await;
                    self.effects.remove(&effect_id);
                }
            }
        }
    }

    pub async fn remove_effects_by_source(&mut self, source_card: &Arc<Mutex<Card>>) {
        let mut effect_ids_to_remove = Vec::new();

        // Collect effect entries to avoid holding a mutable borrow on self.effects
        let effect_entries: Vec<(EffectID, Arc<Mutex<dyn Effect + Send + Sync>>)> = self
            .effects
            .iter()
            .map(|(effect_id, effect_arc)| (effect_id.clone(), Arc::clone(effect_arc)))
            .collect();

        for (effect_id, effect_arc) in effect_entries {
            let mut effect = effect_arc.lock().await;
            if let Some(effect_source_card) = effect.get_source_card() {
                if Arc::ptr_eq(effect_source_card, source_card) {
                    // Call cleanup before removing
                    effect.cleanup().await;
                    effect_ids_to_remove.push(effect_id);
                }
            }
        }

        // Remove effects after iteration
        for effect_id in effect_ids_to_remove {
            self.effects.remove(&effect_id);
        }
    }
}

impl fmt::Debug for EffectManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectManager").finish()
    }
}

#[derive(Debug)]
pub struct StatModifierEffect {
    pub target: EffectTarget,
    pub stat_type: StatType,
    pub amount: i8,
    pub remaining_turns: Option<u32>, // None for permanent effects
    pub id: EffectID,
    pub applied: bool,
    pub source_card: Option<Arc<Mutex<Card>>>,
}

impl StatModifierEffect {
    pub fn new(
        target: EffectTarget,
        stat_type: StatType,
        amount: i8,
        remaining_turns: Option<u32>,
        source_card: Option<Arc<Mutex<Card>>>,
    ) -> StatModifierEffect {
        StatModifierEffect {
            target,
            stat_type,
            source_card,
            amount,
            remaining_turns,
            id: EffectID::new(),
            applied: false,
        }
    }
}

#[async_trait::async_trait]
impl Effect for StatModifierEffect {
    fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>> {
        self.source_card.as_ref()
    }
    async fn apply(&mut self) {
        if !self.applied {
            let id_str = self.id.to_string();
            match &self.target {
                EffectTarget::Card(card_arc) => {
                    let mut card = card_arc.lock().await;
                    card.stats
                        .add_stat(id_str.clone(), Stat::new(self.stat_type, self.amount));
                }
                EffectTarget::Player(player_arc) => {
                    let mut player = player_arc.lock().await;
                    player
                        .stat_manager
                        .add_stat(id_str.clone(), Stat::new(self.stat_type, self.amount));
                }
            }
            self.applied = true;
        }

        // Decrement duration if applicable
        if let Some(remaining) = &mut self.remaining_turns {
            if *remaining > 0 {
                *remaining -= 1;
            }
        }
    }

    fn is_expired(&self) -> bool {
        self.remaining_turns
            .map_or(false, |remaining| remaining == 0)
    }

    async fn cleanup(&mut self) {
        let id_str = self.id.to_string();
        match &self.target {
            EffectTarget::Card(card_arc) => {
                let mut card = card_arc.lock().await;
                card.stats.remove_stat(id_str);
            }
            EffectTarget::Player(player_arc) => {
                let mut player = player_arc.lock().await;
                player.stat_manager.remove_stat(id_str);
            }
        }
    }

    fn get_id(&self) -> &EffectID {
        &self.id
    }
}

pub struct ApplyEffectToTargetAction {
    pub effect_generator: Arc<
        dyn Fn(EffectTarget, Option<Arc<Mutex<Card>>>) -> Arc<Mutex<dyn Effect + Send + Sync>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyEffectToTargetAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToTargetAction").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectToTargetAction {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>) {
        let target = {
            let card = card_arc.lock().await;
            card.target.clone()
        }; // Lock is released here

        if let Some(target) = target {
            let source_card = Some(Arc::clone(&card_arc)); // Set the source card
            let effect = (self.effect_generator)(target, source_card);
            let effect_id = {
                let effect_lock = effect.lock().await;
                effect_lock.get_id().clone()
            }; // Lock is released here

            game.effect_manager.add_effect(effect_id, effect);
        } else {
            println!("No target specified for card action.");
        }
    }
}
