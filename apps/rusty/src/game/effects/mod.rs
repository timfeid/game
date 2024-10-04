use fmt::Debug;
use std::{any::Any, collections::HashMap, fmt, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    action::{CardAction, CardActionTarget},
    card::{Card, CardType},
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::{Turn, TurnPhase},
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
    async fn apply(&mut self, turn: Turn);
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

    pub async fn apply_effects(&mut self, turn: Turn) {
        let effect_ids: Vec<EffectID> = self.effects.keys().cloned().collect();
        for effect_id in effect_ids {
            if let Some(effect_arc) = self.effects.clone().get(&effect_id) {
                let mut effect = effect_arc.lock().await;
                effect.apply(turn.clone()).await;
                if effect.is_expired() {
                    println!("cleaning up effect.");
                    effect.cleanup().await;
                    self.effects.remove(&effect_id);
                }
            }
        }
    }

    pub async fn has_effects(&self, source_card: &Arc<Mutex<Card>>) -> bool {
        let effect_entries: Vec<(EffectID, Arc<Mutex<dyn Effect + Send + Sync>>)> = self
            .effects
            .iter()
            .map(|(effect_id, effect_arc)| (effect_id.clone(), Arc::clone(effect_arc)))
            .collect();

        for (effect_id, effect_arc) in effect_entries {
            let effect = effect_arc.lock().await;
            if let Some(effect_source_card) = effect.get_source_card() {
                if Arc::ptr_eq(effect_source_card, source_card) {
                    return true;
                }
            }
        }

        false
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
pub enum ExpireContract {
    Turns(i8),
    Never,
}

#[derive(Debug)]
pub struct StatModifierEffect {
    pub target: EffectTarget,
    pub stat_type: StatType,
    pub amount: i8,
    pub expires: ExpireContract, // None for permanent effects
    pub id: EffectID,
    pub applied: bool,
    pub source_card: Option<Arc<Mutex<Card>>>,
    previous_turn: Option<i32>,
}

impl StatModifierEffect {
    pub fn new(
        target: EffectTarget,
        stat_type: StatType,
        amount: i8,
        expires: ExpireContract,
        source_card: Option<Arc<Mutex<Card>>>,
    ) -> StatModifierEffect {
        StatModifierEffect {
            target,
            stat_type,
            source_card,
            amount,
            expires,
            id: EffectID::new(),
            applied: false,
            previous_turn: None,
        }
    }
}

#[async_trait::async_trait]
impl Effect for StatModifierEffect {
    fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>> {
        self.source_card.as_ref()
    }
    async fn apply(&mut self, turn: Turn) {
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
        match &mut self.expires {
            ExpireContract::Turns(remaining) => {
                if let Some(prev) = self.previous_turn {
                    if prev != turn.turn_number && *remaining > 0 {
                        *remaining -= 1;
                        println!("remaining {:?}", remaining);
                    }
                } else {
                    self.previous_turn = Some(turn.turn_number);
                }
            }
            _ => {}
        }
    }

    fn is_expired(&self) -> bool {
        // Decrement duration if applicable
        match &self.expires {
            ExpireContract::Turns(remaining) => *remaining == 0,
            _ => false,
        }
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

pub struct ApplyEffectToCardBasedOnTotalCardType {
    pub card_type: CardType,
    pub effect_generator: Arc<
        dyn Fn(EffectTarget, Option<Arc<Mutex<Card>>>, i8) -> Arc<Mutex<dyn Effect + Send + Sync>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyEffectToCardBasedOnTotalCardType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToCardBasedOnTotalCardType")
            .finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectToCardBasedOnTotalCardType {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>) {
        let mut total = 0;
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(owner_arc) = owner_arc {
            let owner = owner_arc.lock().await;

            for card_in_play in &owner.cards_in_play {
                let card_type_matches = {
                    let card = card_in_play.lock().await;
                    card.card_type == self.card_type
                };

                if card_type_matches {
                    total = total + 1;
                }
            }
        } else {
            println!("No owner found for the card.");
        }

        println!("Found {} total cards matching {:?}", total, self.card_type);

        if total > 0 {
            let source_card = Some(Arc::clone(&card_arc));
            let target = &card_arc.lock().await.attached;
            if let Some(x) = target {
                let effect =
                    (self.effect_generator)(EffectTarget::Card(Arc::clone(x)), source_card, total);
                let effect_id = {
                    let effect_lock = effect.lock().await;
                    effect_lock.get_id().clone()
                };

                game.effect_manager.add_effect(effect_id, effect);
            }
        }
    }
}

pub struct ApplyEffectToPlayerCardType {
    pub card_type: CardType,
    pub effect_generator: Arc<
        dyn Fn(EffectTarget, Option<Arc<Mutex<Card>>>) -> Arc<Mutex<dyn Effect + Send + Sync>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyEffectToPlayerCardType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToPlayerCardType").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectToPlayerCardType {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>) {
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(owner_arc) = owner_arc {
            let owner = owner_arc.lock().await;

            // Iterate over the owner's in-play cards and apply the effect to matching card types
            for card_in_play in &owner.cards_in_play {
                let card_type_matches = {
                    let card = card_in_play.lock().await;
                    card.card_type == self.card_type
                };

                if card_type_matches {
                    let source_card = Some(Arc::clone(&card_arc));
                    let effect = (self.effect_generator)(
                        EffectTarget::Card(Arc::clone(&card_in_play)),
                        source_card,
                    );
                    let effect_id = {
                        let effect_lock = effect.lock().await;
                        effect_lock.get_id().clone()
                    }; // Lock is released here

                    println!("Adding effect {:?} to card {:?}", effect, card_arc);

                    game.effect_manager.add_effect(effect_id, effect);
                } else {
                    // println!("Card does not match card type {:?}", self.card_type);
                }
            }
        } else {
            println!("No owner found for the card.");
        }
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
            let effect = (self.effect_generator)(target.clone(), source_card);
            let effect_id = {
                let effect_lock = effect.lock().await;
                effect_lock.get_id().clone()
            }; // Lock is released here
            println!("Adding effect {:?} to card {:?}", effect, target);

            game.effect_manager.add_effect(effect_id, effect);
        } else {
            println!("No target specified for card action.");
        }
    }
}

#[derive(Debug)]
pub struct DrawCardCardAction {
    pub target: CardActionTarget,
}

#[async_trait::async_trait]
impl CardAction for DrawCardCardAction {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>) {
        let card = card_arc.lock().await;
        let owner = card.owner.as_ref().unwrap();
        owner.lock().await.draw_card();
    }
}
