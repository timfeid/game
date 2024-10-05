use fmt::Debug;
use std::{any::Any, collections::HashMap, fmt, future::Future, pin::Pin, sync::Arc};
use tokio::sync::Mutex;
use ulid::Ulid;
use uuid::Uuid;

use super::{
    action::{ActionTriggerType, CardAction, CardActionTarget},
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
pub struct EffectID(String);

impl EffectID {
    pub fn new() -> Self {
        EffectID(Uuid::new_v4().to_string())
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

// Trait for defining different types of effects
#[async_trait::async_trait]
pub trait Effect: Send + Sync + Debug {
    async fn apply(&mut self, turn: Turn);
    // fn is_permenant(&self) -> bool {
    //     false
    // }
    fn is_expired(&self) -> bool {
        false
    }
    async fn cleanup(&mut self) {}
    fn get_id(&self) -> &EffectID;
    fn get_final_id(&self) -> EffectID {
        self.get_id().clone()
    }
    fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>> {
        None
    }
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
        println!("Apply effects called!");
        let effect_ids: Vec<EffectID> = self.effects.keys().cloned().collect();
        for effect_id in effect_ids {
            if let Some(effect_arc) = self.effects.clone().get(&effect_id) {
                let mut effect = effect_arc.lock().await;
                println!(
                    "Applying effect for card {}",
                    effect.get_source_card().clone().unwrap().lock().await.name
                );
                {
                    effect.apply(turn.clone()).await;
                }
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
            let id = self.get_final_id().to_string();
            match &self.target {
                EffectTarget::Card(card_arc) => {
                    let mut card = card_arc.lock().await;
                    card.stats
                        .add_stat(id, Stat::new(self.stat_type, self.amount));
                }
                EffectTarget::Player(player_arc) => {
                    let mut player = player_arc.lock().await;
                    player
                        .stat_manager
                        .add_stat(id, Stat::new(self.stat_type, self.amount));
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
        let id_str = self.get_id().to_string();
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

pub struct DynamicStatModifierEffect {
    pub target: EffectTarget,
    pub stat_type: StatType,
    pub amount_calculator:
        Arc<dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i8> + Send>> + Send + Sync>,
    pub expires: ExpireContract,
    pub id: EffectID,
    pub applied: bool,
    pub source_card: Option<Arc<Mutex<Card>>>,
    pub permanent_change: bool,
}

impl Debug for DynamicStatModifierEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicStatModifierEffect")
            .field("target", &self.target)
            .field("stat_type", &self.stat_type)
            .field("expires", &self.expires)
            .field("id", &self.id)
            .field("applied", &self.applied)
            .field("source_card", &self.source_card)
            .finish()
    }
}

impl DynamicStatModifierEffect {
    pub fn new(
        target: EffectTarget,
        stat_type: StatType,
        amount_calculator: Arc<
            dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i8> + Send>> + Send + Sync,
        >,
        expires: ExpireContract,
        source_card: Option<Arc<Mutex<Card>>>,
        permanent_change: bool,
    ) -> Self {
        Self {
            target,
            stat_type,
            amount_calculator,
            expires,
            id: EffectID::new(),
            applied: false,
            source_card,
            permanent_change,
        }
    }
}

#[async_trait::async_trait]
impl Effect for DynamicStatModifierEffect {
    fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>> {
        self.source_card.as_ref()
    }

    fn get_id(&self) -> &EffectID {
        &self.id
    }

    // fn get_final_id(&self) -> EffectID {
    //     if self.permanent_change {
    //         println!("it's a permenant change");
    //         let mut id = self.id.to_string();
    //         id.insert_str(
    //             0,
    //             format!("{:?}-{}", self.get_source_card(), Ulid::new()).as_str(),
    //         );
    //         EffectID(id)
    //     } else {
    //         self.get_id().clone()
    //     }
    // }

    async fn apply(&mut self, turn: Turn) {
        // Recalculate the amount
        if let Some(source_card) = self.source_card.clone() {
            let amount = (self.amount_calculator)(source_card).await;
            println!("AMOUNT TO APPLY: {}", amount);
            let id = self.get_final_id().to_string();

            // Apply the stat modification
            match &self.target {
                EffectTarget::Card(card_arc) => {
                    let mut card = card_arc.lock().await;
                    if self.permanent_change {
                        card.stats.modify_stat(self.stat_type, amount);
                    } else {
                        card.stats.add_stat(id, Stat::new(self.stat_type, amount));
                    }
                }
                EffectTarget::Player(player_arc) => {
                    let mut player = player_arc.lock().await;
                    if self.permanent_change {
                        player.stat_manager.modify_stat(self.stat_type, amount);
                    } else {
                        player
                            .stat_manager
                            .add_stat(id, Stat::new(self.stat_type, amount));
                    }
                }
            }
            self.applied = true;
        }
    }

    fn is_expired(&self) -> bool {
        match &self.expires {
            ExpireContract::Turns(remaining) => *remaining == 0,
            _ => false,
        }
    }
}

// #[derive(Debug)]
// pub struct LifeDrainEffect {
//     pub target: EffectTarget,
//     pub amount: i8,
//     pub expires: ExpireContract,
//     pub id: EffectID,
//     pub applied: bool,
//     pub source_card: Option<Arc<Mutex<Card>>>,
// }

// impl LifeDrainEffect {
//     pub fn new(
//         target: EffectTarget,
//         amount: i8,
//         expires: ExpireContract,
//         source_card: Option<Arc<Mutex<Card>>>,
//     ) -> Self {
//         LifeDrainEffect {
//             target,
//             amount,
//             expires,
//             id: EffectID::new(),
//             applied: false,
//             source_card,
//         }
//     }
// }

// #[async_trait::async_trait]
// impl Effect for LifeDrainEffect {
//     fn get_source_card(&self) -> Option<&Arc<Mutex<Card>>> {
//         self.source_card.as_ref()
//     }

//     fn get_id(&self) -> &EffectID {
//         &self.id
//     }

//     async fn apply(&mut self, turn: Turn) {
//         match &self.target {
//             EffectTarget::Player(player_arc) => {
//                 let mut id = self.get_id().to_string();
//                 id.insert_str(0, "lifelink");
//                 let mut player = player_arc.lock().await;
//                 player.add_stat(id, Stat::new(StatType::Health, self.amount));
//             }
//             _ => {}
//         }

//         if let Some(source_card_arc) = &self.source_card {
//             // Assuming the source card's owner is the one gaining life
//             let owner_arc = {
//                 let card = source_card_arc.lock().await;
//                 card.owner.clone()
//             };

//             if let Some(owner_arc) = owner_arc {
//                 let mut owner = owner_arc.lock().await;
//                 let mut id = self.get_id().to_string();
//                 id.insert_str(0, "lifelink");
//                 let mut player = owner_arc.lock().await;
//                 player.add_stat(id, Stat::new(StatType::Health, self.amount));
//             }
//         }

//         self.applied = true;
//     }

//     async fn cleanup(&mut self) {
//         // Life drain is irreversible; nothing to remove
//         self.applied = false;
//     }

//     fn is_expired(&self) -> bool {
//         matches!(self.expires, ExpireContract::Turns(0))
//     }
// }

#[derive(Debug)]
pub struct LifeLinkAction {}

#[async_trait::async_trait]
impl CardAction for LifeLinkAction {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        let (owner, amount) = {
            let lock = card_arc.lock().await;
            let owner = lock.owner.clone();
            let amount = lock.damage_dealt_to_players.clone();
            (owner, amount)
        };
        if let Some(owner) = owner {
            owner
                .lock()
                .await
                .stat_manager
                .add_stat(Ulid::new().to_string(), Stat::new(StatType::Health, amount));
        }
    }
}
pub struct ApplyDynamicEffectToCard {
    pub amount_calculator:
        Arc<dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i8> + Send>> + Send + Sync>,
    pub effects_generator: Arc<
        dyn Fn(
                EffectTarget,
                Arc<Mutex<Card>>,
                Option<Arc<Mutex<Player>>>,
                Arc<
                    dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i8> + Send>>
                        + Send
                        + Sync,
                >,
            ) -> Vec<Arc<Mutex<dyn Effect + Send + Sync>>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyDynamicEffectToCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyDynamicEffectToCard").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyDynamicEffectToCard {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        let amount_calculator = Arc::clone(&self.amount_calculator);
        let amount = (&amount_calculator)(Arc::clone(&card_arc)).await;
        if amount == 0 {
            println!("hmmm, 0 amount?zc:");
        }
        let effects = {
            let source_card = Arc::clone(&card_arc);

            let owner = { &source_card.lock().await.owner.clone() };

            (self.effects_generator)(target, source_card, owner.clone(), amount_calculator)
        };

        for effect in effects {
            let effect_id = effect.lock().await.get_final_id();
            println!("received effect from list {:?}", effect_id);

            game.effect_manager.add_effect(effect_id, effect);
        }
    }
}

pub struct ApplyEffectToCardBasedOnTotalCardType {
    pub card_type: CardType,
    pub effect_generator: Arc<
        dyn Fn(
                EffectTarget,
                Option<Arc<Mutex<Card>>>,
                Arc<
                    dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i8> + Send>>
                        + Send
                        + Sync,
                >,
            ) -> Arc<Mutex<dyn Effect + Send + Sync>>
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
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        println!(
            "effect target for dynamic effect\n\n\ncard: {:?}\ntarget: {:?}\n\n\n",
            card_arc, target
        );
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(_) = owner_arc {
            let source_card = Some(Arc::clone(&card_arc));
            let target = { card_arc.lock().await.attached.clone() };

            if let Some(target_arc) = target {
                let card_type = self.card_type;
                let amount_calculator = {
                    Arc::new(move |card_arc: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = i8> + Send>> {
                        Box::pin(async move {
                            let mut total = 0;

                            let (card_type, owner) = {
                                let card =card_arc.lock().await;
                                let owner = card.owner.clone();
                                ( card_type, owner )
                            };

                            if let Some(owner_arc) = owner {

                                let owner = owner_arc.lock().await;
                                for card_in_play in &owner.cards_in_play {
                                    let card_type_matches = {
                                        let card = card_in_play.lock().await;
                                        card.card_type == card_type
                                    };

                                    if card_type_matches {
                                        total += 1;
                                    }
                                }
                            }
                            total
                        })
                    })
                };

                let effect = (self.effect_generator)(
                    EffectTarget::Card(target_arc),
                    source_card,
                    amount_calculator,
                );

                let effect_id = effect.lock().await.get_final_id();

                game.effect_manager.add_effect(effect_id, effect);
            } else {
                println!("OH NO, NO ATTACHED CARD ON {}", &card_arc.lock().await.name);
            }
        } else {
            println!("No owner found for the card.");
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
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        println!(
            "effect target for dynamic effect\n\n\ncard: {:?}\ntarget: {:?}\n\n\n",
            card_arc, target
        );
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
                    let effect_id = effect.lock().await.get_final_id();

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
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        let target = {
            let card = card_arc.lock().await;
            card.target.clone()
        }; // Lock is released here

        if let Some(target) = target {
            let source_card = Some(Arc::clone(&card_arc)); // Set the source card
            let effect = (self.effect_generator)(target.clone(), source_card);
            let effect_id = effect.lock().await.get_final_id();
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
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        let card = card_arc.lock().await;
        let owner = card.owner.as_ref().unwrap();
        owner.lock().await.draw_card();
    }
}
