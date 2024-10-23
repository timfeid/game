use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;
use std::vec;
use std::{borrow::Borrow, cell::RefCell};

use serde::{Deserialize, Serialize};
use specta::Type;
use textwrap::fill;
use tokio::sync::{Mutex, MutexGuard, TryLockError};
use ulid::Ulid;

use crate::error::AppError;
use crate::game::action;
use crate::game::effects::{EffectManager, EffectTarget};
use crate::lobby::manager::AbilityDetails;

use super::action::{
    ActionBuilder, ActionTriggerType, Attachable, CardAction, CardActionTarget, CardActionTrigger,
    CardActionWrapper, CardRequiredTarget, DeclareAttackerAction, DeclareBlockerAction,
    PhaseTarget, PlayCardAction, PlayerAction, PlayerActionTrigger, ResetCardAction,
};

use super::effects::{EffectID, ExpireContract, StatModifierEffect};
use super::mana::ManaType;
use super::turn::Turn;
use super::{
    action::Action,
    player::Player,
    stat::{Stat, StatManager, StatType, Stats},
    turn::TurnPhase,
    Game,
};
use super::{player, ActionType, FrontendTarget};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Type)]
pub enum CreatureType {
    None,
    Angel,
    Elf,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Type)]
pub enum CardType {
    Creature,
    Planeswalker,
    Enchantment,
    Instant,
    Sorcery,
    Artifact,
    BasicLand(ManaType),
    AdvancedLand(ManaType),
    AdvancedMultiLand(ManaType, ManaType),
    // Land(Vec<ManaType>),
}

impl CardType {
    pub fn is_spell(&self) -> bool {
        match self {
            CardType::BasicLand(_) => false,
            CardType::AdvancedLand(_) => false,
            CardType::AdvancedMultiLand(_, _) => false,
            // CardType::Land(_) => false,
            _ => true,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Hash, Type)]
pub enum CardPhase {
    Charging(u8),
    Ready,
    Complete,
    Cancelled,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Hash, Type)]
pub enum Counter {
    PowerToughnessModifier(i16, i16),
    Incremental(i16),
}

pub struct CardBuilder {
    name: String,
    description: String,
    actions: Vec<CardActionTrigger>,
    mana_cost: Vec<ManaType>,
    card_type: CardType,
    phase: CardPhase,
    stats: Vec<Stat>,
    creature_type: Option<CreatureType>,
    play_restrictions: Option<(Vec<TurnPhase>, PhaseTarget)>,
}

impl CardBuilder {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            actions: vec![],
            mana_cost: vec![],
            card_type: CardType::Sorcery,
            phase: CardPhase::Ready,
            stats: vec![],
            creature_type: None,
            play_restrictions: Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn add_action(mut self, action: ActionBuilder) -> Self {
        self.actions.push(action.build());
        self
    }

    pub fn play_restrictions(mut self, phases: Vec<TurnPhase>, player: PhaseTarget) -> Self {
        self.play_restrictions = Some((phases, player));
        self
    }

    pub fn mana_cost(mut self, mana: Vec<ManaType>) -> Self {
        self.mana_cost = mana;
        self
    }

    pub fn card_type(mut self, card_type: CardType) -> Self {
        self.card_type = card_type;
        self
    }

    pub fn phase(mut self, phase: CardPhase) -> Self {
        self.phase = phase;
        self
    }

    pub fn build(self) -> Card {
        Card::new(
            &self.name,
            &self.description,
            self.actions,
            self.phase,
            self.card_type,
            self.stats,
            self.mana_cost,
            self.play_restrictions,
        )
    }

    pub fn creature(mut self, power: i16, toughness: i16) -> Self {
        self.stats.push(Stat::new(StatType::Power, power));
        self.stats.push(Stat::new(StatType::Toughness, toughness));
        self = self.card_type(CardType::Creature);
        self = self.add_action(
            ActionBuilder::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Attack".to_string(),
                    vec![],
                    Some((vec![TurnPhase::DeclareAttackers], PhaseTarget::Owner)),
                    true,
                ),
                CardRequiredTarget::EnemyCardOrPlayer,
            )
            .action(DeclareAttackerAction {}),
        );
        self = self.add_action(
            ActionBuilder::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Block".to_string(),
                    vec![],
                    Some((vec![TurnPhase::DeclareBlockers], PhaseTarget::Any)),
                    true,
                ),
                CardRequiredTarget::EnemyCardInCombat,
            )
            .action(DeclareAttackerAction {}),
        );

        self
    }

    pub fn creature_of_type(
        mut self,
        power: i16,
        toughness: i16,
        creature_type: CreatureType,
    ) -> Self {
        self = self.creature(power, toughness);
        self.creature_type = Some(creature_type);

        self
    }
}

#[derive(Type, Debug, Deserialize, Serialize, Clone)]
pub struct Card {
    pub play_restrictions: Option<(Vec<TurnPhase>, PhaseTarget)>,
    pub creature_type: Option<CreatureType>,
    pub name: String,
    pub description: String,
    pub card_type: CardType,
    pub current_phase: CardPhase,
    #[serde(skip_serializing, skip_deserializing)]
    pub target: Option<FrontendTarget>,
    pub tapped: bool,
    pub stats: StatManager,
    #[serde(skip_serializing, skip_deserializing)]
    pub triggers: Vec<CardActionTrigger>,
    pub cost: Vec<ManaType>,
    #[serde(skip_serializing, skip_deserializing)]
    pub owner: Option<Arc<Mutex<Player>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub attached: Option<Arc<Mutex<Card>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub damage_dealt_to_players: i16,
    #[serde(skip_serializing, skip_deserializing)]
    pub damage_taken: i16,
    pub is_countered: bool,
    pub id: String,
    pub counters: HashMap<String, Counter>,
}

impl Card {
    pub fn abilities(
        &self,
        turn_phase: TurnPhase,
        in_play: bool,
        original_card_arc: Option<Arc<Mutex<Card>>>,
        game_arc: Option<Arc<Mutex<Game>>>,
    ) -> Vec<AbilityDetails> {
        let mut abilities = vec![];
        // if self.current_phase == CardPhase::Exiled {
        //     return abilities;
        // }

        for trigger in self.triggers.iter() {
            match &trigger.trigger_type {
                // action::ActionTriggerType::CardTapped => {
                //     return (trigger.card_required_target.clone(), ActionType::Tap)
                // }
                // action::ActionTriggerType::CardTappedWithinPhases(allowed_phases) => {
                //     if allowed_phases.contains(&turn_phase) {
                //         return (trigger.card_required_target.clone(), ActionType::Tap);
                //     }
                // }
                // action::ActionTriggerType::CardEnteredBattlefield => {
                //     let mut is_owner = !in_play;

                //     if let Some(game) = &game_arc {
                //         is_owner = Arc::ptr_eq(
                //             &self.owner.clone().unwrap(),
                //             &game
                //                 .try_lock()
                //                 .expect("Unable to grab game")
                //                 .current_turn
                //                 .as_ref()
                //                 .unwrap()
                //                 .current_player,
                //         );
                //     }
                //     let within_phase = restrictions
                //         .as_ref()
                //         .and_then(|(phase, trigger_target)| {
                //             Some(
                //                 phase.contains(&turn_phase)
                //                     && match trigger_target {
                //                         PhaseTarget::Owner => is_owner,
                //                         PhaseTarget::Opponent => !is_owner,
                //                         PhaseTarget::Any => true,
                //                     },
                //             )
                //         })
                //         .unwrap_or(true);

                //     let mut meets_requirements_except_mana = within_phase && !in_play;
                //     if let Some(game_arc) = &game_arc {
                //         if let Some(card) = &original_card_arc {
                //             if let Ok(game) = game_arc.try_lock() {
                //                 if let Ok(card) = card.try_lock() {
                //                     meets_requirements_except_mana = meets_requirements_except_mana
                //                         && (&trigger.requirements)(
                //                             &game,
                //                             &card,
                //                             Some(trigger.id.clone()),
                //                         );
                //                 }
                //             }
                //         }
                //     }
                //     let mut meets_mana_requirements = false;
                //     let mut can_pay_mana = false;
                //     let mut player_id = None;

                //     if let Some(owner) = &self.owner {
                //         if let Ok(owner) = owner.try_lock() {
                //             meets_mana_requirements = owner.has_required_mana(&self.cost);
                //             can_pay_mana = owner.can_pay_mana(&self.cost);
                //             player_id = Some(owner.name.clone());
                //         }
                //     }

                //     added_play_from_hand_action = true;
                //     if let Some(card) = &original_card_arc {
                //         abilities.push(AbilityDetails {
                //             id: "play_card".to_string(),
                //             action_type: ActionType::PlayedCard,
                //             mana_cost: vec![],
                //             required_target: trigger.card_required_target.clone(),
                //             description: "Play".to_string(),
                //             show: false,
                //             meets_requirements_except_mana,
                //             meets_mana_requirements,
                //             can_pay_mana,
                //             owner_player_id: player_id,
                //             tap_required: false,
                //             action: Some(Arc::new(PlayCardAction {})),
                //         });
                //     }
                // }
                action::ActionTriggerType::Attached => {
                    let mut player_id = None;

                    if let Some(owner) = &self.owner {
                        if let Ok(owner) = owner.try_lock() {
                            player_id = Some(owner.name.clone());
                        }
                    }

                    if &turn_phase == &TurnPhase::Main {
                        abilities.push(AbilityDetails {
                            id: trigger.id.clone(),
                            action_type: ActionType::Attach,
                            mana_cost: vec![],
                            required_target: trigger.card_required_target.clone(),
                            description: "Attach".to_string(),
                            show: in_play,
                            meets_requirements_except_mana: in_play,
                            meets_mana_requirements: true,
                            can_pay_mana: true,
                            owner_player_id: player_id.clone(),
                            tap_required: false,
                            action: Some(trigger.action.clone()),
                        });
                    }
                }

                action::ActionTriggerType::AbilityWithinPhases(
                    description,
                    required_mana,
                    phase_restrictions,
                    required_tap,
                ) => {
                    let mut is_owner = false;
                    if let Some(game) = &game_arc {
                        if let Some(current_player) = game
                            .try_lock()
                            .expect("unable to lock player")
                            .current_turn
                            .as_ref()
                            .map(|t| t.current_player.clone())
                        {
                            is_owner = Arc::ptr_eq(self.owner.as_ref().unwrap(), &current_player);
                        }
                    }
                    let main_phase_restriction = phase_restrictions
                        .as_ref()
                        .and_then(|(phase, trigger_target)| {
                            Some(
                                phase.contains(&TurnPhase::Main)
                                    || phase.contains(&TurnPhase::Main2),
                            )
                        })
                        .unwrap_or(false);
                    let within_phase = phase_restrictions
                        .as_ref()
                        .and_then(|(phase, trigger_target)| {
                            Some(
                                phase.contains(&turn_phase)
                                    && match trigger_target {
                                        PhaseTarget::Owner => is_owner,
                                        PhaseTarget::Opponent => !is_owner,
                                        PhaseTarget::Any => true,
                                    },
                            )
                        })
                        .unwrap_or(true);

                    let can_pay_mana = self
                        .owner
                        .as_ref()
                        .and_then(|o| {
                            Some(
                                o.try_lock()
                                    .and_then(|o| Ok(o.can_pay_mana(required_mana)))
                                    .unwrap_or(false),
                            )
                        })
                        .unwrap_or(false);

                    let player_id = self.owner.as_ref().and_then(|o| {
                        Some(
                            o.try_lock()
                                .and_then(|o| Ok(o.name.clone()))
                                .unwrap_or_default(),
                        )
                    });

                    let meets_mana_requirements = self
                        .owner
                        .as_ref()
                        .and_then(|o| {
                            Some(
                                o.try_lock()
                                    .and_then(|o| Ok(o.has_required_mana(required_mana)))
                                    .unwrap_or(false),
                            )
                        })
                        .unwrap_or(false);
                    // if can_pay_mana && within_phase {
                    let mut meets_requirements_except_mana = within_phase
                        && in_play
                        && ((!self.tapped && self.current_phase == CardPhase::Ready)
                            || !required_tap);

                    if let Some(card) = &original_card_arc {
                        if let Some(game) = &game_arc {
                            if let Ok(card) = card.try_lock() {
                                meets_requirements_except_mana = meets_requirements_except_mana
                                    && (&trigger.requirements)(
                                        game,
                                        &card,
                                        Some(trigger.id.clone()),
                                    );
                            }
                        }
                    }

                    abilities.push(AbilityDetails {
                        id: trigger.id.clone(),
                        mana_cost: required_mana.clone(),
                        required_target: trigger.card_required_target.clone(),
                        description: description.to_string(),
                        action_type: if *required_tap {
                            ActionType::Tap
                        } else {
                            ActionType::Instant
                        },
                        show: meets_requirements_except_mana
                            || phase_restrictions.is_none()
                            || main_phase_restriction,
                        meets_requirements_except_mana,
                        meets_mana_requirements,
                        can_pay_mana,
                        owner_player_id: player_id,
                        tap_required: required_tap.clone(),
                        action: Some(trigger.action.clone()),
                    });

                    // return (trigger.card_required_target.clone(), action_type);
                    // }
                }
                x => {}
            }
        }

        if !in_play {
            let mut meets_mana_requirements = false;
            let mut can_pay_mana = false;
            let mut player_id = None;
            let mut is_owner = !in_play;

            if let Some(game) = &game_arc {
                is_owner = Arc::ptr_eq(
                    &self.owner.clone().unwrap(),
                    &game
                        .try_lock()
                        .unwrap()
                        .current_turn
                        .as_ref()
                        .unwrap()
                        .current_player,
                );
            }

            let within_phase = self
                .play_restrictions
                .clone()
                .and_then(|(phase, trigger_target)| {
                    Some(
                        phase.contains(&turn_phase)
                            && match trigger_target {
                                PhaseTarget::Owner => is_owner,
                                PhaseTarget::Opponent => !is_owner,
                                PhaseTarget::Any => true,
                            },
                    )
                })
                .unwrap_or(true);
            let meets_requirements_except_mana = within_phase;
            if let Some(owner) = &self.owner {
                if let Ok(owner) = owner.try_lock() {
                    meets_mana_requirements = owner.has_required_mana(&self.cost);
                    can_pay_mana = owner.can_pay_mana(&self.cost);
                    player_id = Some(owner.name.clone());
                }
            }
            abilities.push(AbilityDetails {
                id: "play_card".to_string(),
                action_type: ActionType::PlayedCard,
                mana_cost: vec![],
                required_target: CardRequiredTarget::None,
                description: "Play".to_string(),
                show: false,
                meets_requirements_except_mana,
                meets_mana_requirements,
                can_pay_mana,
                owner_player_id: player_id,
                tap_required: false,
                action: Some(Arc::new(PlayCardAction {})),
            });
        }

        abilities
    }
    pub fn new(
        name: &str,
        description: &str,
        triggers: Vec<CardActionTrigger>,
        phase: CardPhase,
        card_type: CardType,
        stats: Vec<Stat>,
        cost: Vec<ManaType>,
        play_restrictions: Option<(Vec<TurnPhase>, PhaseTarget)>,
    ) -> Self {
        let mut card = Self {
            id: Ulid::new().to_string(),
            creature_type: None,
            name: name.to_string(),
            description: description.to_string(),
            tapped: false,
            card_type,
            triggers,
            current_phase: phase,
            target: None,
            stats: StatManager::new(stats),
            cost,
            owner: None,
            attached: None,
            damage_taken: 0,
            damage_dealt_to_players: 0,
            is_countered: false,
            counters: HashMap::new(),
            play_restrictions,
        };

        card
    }

    pub async fn add_counter(card: Arc<Mutex<Self>>, game: &Arc<Mutex<Game>>, counter: Counter) {
        let id = Card::activate_counter(card.clone(), &counter, game).await;
        card.lock().await.counters.insert(id, counter);
        todo!();
        // let mut actions = Game::collect_card_stat_changed_actions(game, &card).await;
        // game.lock().await.execute_actions(&mut actions).await.ok();
    }

    pub async fn activate_counter(
        card: Arc<Mutex<Self>>,
        counter: &Counter,
        game: &Arc<Mutex<Game>>,
    ) -> String {
        let id = Ulid::new().to_string();
        match counter {
            Counter::PowerToughnessModifier(power, toughness) => {
                let mut game = game.lock().await;
                game.effect_manager.add_effect(
                    EffectID(format!("counter-{}-toughness", id)),
                    Arc::new(Mutex::new(StatModifierEffect::new(
                        EffectTarget::Card(card.clone()),
                        StatType::Toughness,
                        toughness.clone(),
                        ExpireContract::Never,
                        None,
                    ))),
                );
                game.effect_manager.add_effect(
                    EffectID(format!("counter-{}-power", id)),
                    Arc::new(Mutex::new(StatModifierEffect::new(
                        EffectTarget::Card(card),
                        StatType::Power,
                        power.clone(),
                        ExpireContract::Never,
                        None,
                    ))),
                );
                println!("applied power toughness counter!");
            }
            Counter::Incremental(total) => {
                card.lock()
                    .await
                    .modify_stat(StatType::Counter, total.clone());
                // card.lock().await.add_stat(
                //     Ulid::new().to_string(),
                //     Stat::new(StatType::Counter, total.clone()),
                // );
            }
        }
        id
    }

    pub fn is_useless(&self, has_effects: bool) -> bool {
        let has_triggers = self
            .triggers
            .iter()
            .filter(|t| match &t.trigger_type {
                ActionTriggerType::AbilityWithinPhases(_, _, _, _) => true,
                ActionTriggerType::PhaseStarted(vec, trigger_target) => true,
                ActionTriggerType::CreatureTypeCardPlayed(trigger_target, creature_type) => true,
                ActionTriggerType::Attached => true,
                ActionTriggerType::DamageApplied => true,
                ActionTriggerType::OtherCardPlayed(_) => true,
                ActionTriggerType::OtherCardDestroyed(_) => true,
                ActionTriggerType::CardExiled => true,
                ActionTriggerType::HealthGained => true,
                ActionTriggerType::CardStatChanged => true,
                ActionTriggerType::OtherCardExiled(trigger_target) => true,

                ActionTriggerType::CardEnteredBattlefield => false,
                ActionTriggerType::Omnipresent => false,
                ActionTriggerType::Detached => false,
                ActionTriggerType::CardDestroyed => false,
            })
            .count()
            > 0;

        !has_triggers && !has_effects
    }

    // pub fn collect_phase_based_actions_sync(
    //     &self,
    //     turn: &Turn,
    //     trigger_type: ActionTriggerType,
    //     card_arc: &Arc<Mutex<Card>>,
    // ) -> Vec<Arc<dyn Action + Send + Sync>> {
    //     let mut phase_based_actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

    //     let owner = match &self.owner {
    //         Some(owner) => Arc::clone(owner),
    //         None => return phase_based_actions,
    //     };

    //     for action_trigger in &self.triggers {
    //         if let ActionTriggerType::PhaseBased(trigger_phase, trigger_target) =
    //             &action_trigger.trigger_type
    //         {
    //             let is_owner = Arc::ptr_eq(&turn.current_player, &owner);
    //             if trigger_phase.contains(&turn.phase)
    //                 && match trigger_target {
    //                     super::action::TriggerTarget::Owner => is_owner,
    //                     super::action::TriggerTarget::Target => !is_owner,
    //                     super::action::TriggerTarget::Any => true,
    //                 }
    //             {
    //                 phase_based_actions.push(Arc::new(CardActionWrapper {
    //                     card: Arc::clone(card_arc),
    //                     action: action_trigger.action.clone(),
    //                     target: match trigger_target {
    //                         action::TriggerTarget::Owner => {
    //                             Some(EffectTarget::Player(Arc::clone(&owner)))
    //                         }
    //                         action::TriggerTarget::Target => {
    //                             card_arc.lock().await.action_target.clone()
    //                         }
    //                         action::TriggerTarget::Any => None,
    //                     },
    //                 }));
    //             }
    //         } else if &trigger_type == &action_trigger.trigger_type {
    //             phase_based_actions.push(Arc::new(CardActionWrapper {
    //                 card: Arc::clone(card_arc),
    //                 action: action_trigger.action.clone(),
    //                 target: None,
    //             }));
    //         }
    //     }

    //     phase_based_actions
    // }

    pub async fn collect_attach_actions(
        &self,
        card_arc: Arc<Mutex<Card>>,
        target: Option<FrontendTarget>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for action_trigger in &self.triggers {
            match &action_trigger.trigger_type {
                ActionTriggerType::Attached => {
                    actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(&card_arc),
                        action: action_trigger.action.clone(),
                        target: target.clone(),
                        ability_id: Some(action_trigger.id.clone()),
                    }));
                }
                _ => {}
            }
        }

        actions
    }

    pub async fn collect_manual_actions(
        card_arc: Arc<Mutex<Card>>,
        in_play: bool,
        target: Option<FrontendTarget>,
        trigger_id: String,
        game: &Arc<Mutex<Game>>,
    ) -> (Vec<Arc<dyn Action + Send + Sync>>, bool, Vec<ManaType>) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let mut requires_tap = false;
        let mut mana_requirements: Vec<ManaType> = vec![];
        let turn_phase = game.lock().await.current_phase();

        let triggers = {
            card_arc.lock().await.abilities(
                turn_phase,
                in_play,
                Some(card_arc.clone()),
                Some(Arc::clone(game)),
            )
        };
        for action_trigger in &triggers {
            if action_trigger.id == trigger_id {
                mana_requirements = action_trigger.mana_cost.clone();

                if action_trigger.meets_mana_requirements
                    && action_trigger.meets_requirements_except_mana
                {
                    requires_tap = action_trigger.tap_required.clone();
                    actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(&card_arc),
                        action: action_trigger.action.clone().expect("No action??"),
                        target: target.clone(),
                        ability_id: Some(action_trigger.id.clone()),
                    }));
                }
                break;
            }
            // match &action_trigger.trigger_type {
            //     ActionTriggerType::AbilityWithinPhases(
            //         _,
            //         mana_requirement,
            //         phase_restrictions,
            //         tap_required,
            //     ) => {
            //         if trigger_id != action_trigger.id {
            //             continue;
            //         }
            //         mana_requirements = mana_requirement.clone();
            //         let in_phases = phase_restrictions.is_none()
            //             || phase_restrictions.as_ref().unwrap().0.contains(&turn_phase);

            //         let meets_requirements = (action_trigger.requirements)(
            //             Arc::clone(&game),
            //             Arc::clone(&card_arc),
            //             trigger_id.clone(),
            //         )
            //         .await;

            //         if in_phases && meets_requirements {
            //             requires_tap = tap_required.clone();
            //             actions.push(Arc::new(CardActionWrapper {
            //                 card: Arc::clone(&card_arc),
            //                 action: action_trigger.action.clone(),
            //                 target: target.clone(),
            //                 ability_id: Some(action_trigger.id.clone()),
            //             }));
            //         }
            //     }
            //     _ => {}
            // }
        }

        (actions, requires_tap, mana_requirements)
    }

    pub async fn collect_card_destroyed_actions(
        card_arc: &Arc<Mutex<Card>>,
        turn: &Turn,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for action_trigger in &card_arc.lock().await.triggers.clone() {
            if let ActionTriggerType::CardDestroyed = &action_trigger.trigger_type {
                actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
                    target: None,
                    ability_id: Some(action_trigger.id.clone()),
                }));
            }
        }

        actions
    }

    pub async fn collect_phase_based_actions(
        card_arc: &Arc<Mutex<Card>>,
        turn: &Turn,
        trigger_type: ActionTriggerType,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut phase_based_actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        let card = card_arc.lock().await;

        let owner = match &card.owner {
            Some(owner) => Arc::clone(owner),
            None => return phase_based_actions,
        };

        for action_trigger in &card.triggers {
            if let ActionTriggerType::PhaseStarted(trigger_phase, phase_target) =
                &action_trigger.trigger_type
            {
                let is_owner = Arc::ptr_eq(&turn.current_player, &owner);
                if trigger_phase.contains(&turn.phase)
                    && match phase_target {
                        super::action::PhaseTarget::Owner => is_owner,
                        super::action::PhaseTarget::Opponent => !is_owner,
                        super::action::PhaseTarget::Any => true,
                    }
                {
                    phase_based_actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(card_arc),
                        action: action_trigger.action.clone(),
                        target: card_arc.lock().await.target.clone(),
                        ability_id: Some(action_trigger.id.clone()),
                    }));
                }
            } else if &trigger_type == &action_trigger.trigger_type {
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
                    target: None,
                    ability_id: Some(action_trigger.id.clone()),
                }));
            }
        }

        phase_based_actions
    }

    pub fn format_mana_cost(&self) -> String {
        let mut formatted_mana = String::new();
        let mut colorless_count = 0;

        for mana in &self.cost {
            match mana {
                ManaType::Colorless => colorless_count += 1,
                _ => {
                    if colorless_count > 0 {
                        formatted_mana.push_str(&format!("{{{}C}} ", colorless_count));
                        colorless_count = 0;
                    }
                    formatted_mana.push_str(&format!("{} ", mana.format()));
                }
            }
        }

        if colorless_count > 0 {
            formatted_mana.push_str(&format!("{{{}C}}", colorless_count));
        }

        formatted_mana.trim_end().to_string()
    }

    pub fn is_tappable(&self) -> bool {
        if self.current_phase != CardPhase::Ready {
            return false;
        }

        if self.tapped {
            return false;
        }

        return true;
    }

    pub fn tap(&mut self) -> Result<(), &str> {
        if self.current_phase != CardPhase::Ready {
            return Err("Card is not ready");
        }

        if self.tapped {
            return Err("Card already tapped");
        }

        println!("{} was tapped", self.name);

        self.tapped = true;

        Ok(())
    }

    // async fn collect_attached_actions(
    //     card: &Arc<Mutex<Card>>,
    //     target_card: &Arc<Mutex<Card>>,
    //     game: &mut Game,
    // ) -> Option<Vec<Arc<dyn Action + Send + Sync>>> {
    //     let attached_actions = Card::collect_phase_based_actions(
    //         card,
    //         &game.current_turn.clone().unwrap(),
    //         ActionTriggerType::Attached,
    //     )
    //     .await;

    //     if attached_actions.is_empty() {
    //         None
    //     } else {
    //         Some(attached_actions)
    //     }
    // }

    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!("┌{}┐", "─".repeat(width - 2)));
        lines.push(format!("│{:^width$}│", self.name, width = width - 2));
        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let mana_costs_content = self.format_mana_cost();
        let stats = format!(
            "{}/{}",
            self.get_stat_value(StatType::Power),
            self.get_stat_value(StatType::Toughness),
        );
        let stats_content = format!(
            " {} {: <width$} {} ",
            mana_costs_content,
            " ",
            stats,
            width = width - stats.len() - mana_costs_content.len() - 6
        );

        lines.push(format!("│{}│", stats_content));
        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let content_width = width - 4;
        let description = fill(&self.description, content_width);
        for desc_line in description.lines() {
            lines.push(format!(
                "│ {: <content_width$}│",
                desc_line,
                content_width = content_width + 1
            ));
        }

        let content_height = 8;
        while lines.len() < content_height {
            lines.push(format!("│{: <width$}│", " ", width = width - 2));
        }

        lines.push(format!(
            "└{}{}┘",
            if self.tapped { "t" } else { "─" },
            "─".repeat(width - 3)
        ));
        lines
    }

    pub(crate) fn untap(&mut self) {
        self.tapped = false;
    }
}

#[async_trait::async_trait]
impl Stats for Card {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stats.add_stat(id, stat);
        println!("{}", self.render(30).join("\n"));
    }

    fn get_stat_value(&self, stat_type: StatType) -> i16 {
        self.stats.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i16) {
        self.stats.modify_stat(stat_type, intensity);
    }

    fn remove_stat(&mut self, id: String) {
        self.stats.remove_stat(id);
    }
}

pub mod card {
    macro_rules! create_multiple_cards {
        ($base_card:expr, $count:expr) => {{
            let mut cards = Vec::new();
            for _ in 0..$count {
                cards.push($base_card.clone()); // Assuming .clone() is implemented
            }
            cards
        }};
    }
    macro_rules! create_planeswalker_card {
        // Base case with additional stats
        ($name:expr, $creature_type:expr, $description:expr, $damage:expr, $defense:expr, [$($mana:expr),*], [$($stat:expr),*] $(, $additional_triggers:expr)*) => {
            {

                let mut card = Card::new(
                    $name,
                    $description,
                    {
                        // Start with the default triggers
                        #[allow(unused_mut)]
                        let mut triggers = vec![
                            // Action to declare the creature as an attacker in the Declare Attackers phase
                            CardActionTrigger::new(
                                ActionTriggerType::AbilityWithinPhases("Attack".to_string(), vec![], Some(vec![TurnPhase::DeclareAttackers]), true),
                                CardRequiredTarget::EnemyCardOrPlayer,
                                Arc::new(DeclareAttackerAction {}),
                            ),
                            // Action to manually declare the creature as a blocker in the Declare Blockers phase
                            CardActionTrigger::new(
                                ActionTriggerType::AbilityWithinPhases("Block".to_string(), vec![], Some(vec![TurnPhase::DeclareBlockers]), false),
                                CardRequiredTarget::EnemyCardInCombat,
                                Arc::new(DeclareBlockerAction {}),
                            ),
                        ];

                        // Add any additional triggers provided
                        $(triggers.push($additional_triggers);)*

                        triggers
                    },
                    // Card starts with a charging phase (this can be customized)
                    CardPhase::Charging(1),
                    // Card type is a Creature
                    CardType::Creature,
                    // Add the specified damage, defense, and additional stats
                    {
                        let mut stats = vec![
                            Stat::new(StatType::Power, $damage),
                            Stat::new(StatType::Toughness, $defense),
                        ];

                        // Add any extra stats (e.g. Trample, Flying)
                        $(stats.push(Stat::new($stat, 1));)*

                        stats
                    },
                    // Specify the mana requirements for the creature card
                    vec![$($mana),*],
                );
                card.creature_type = Some($creature_type);
                card
            }


        };
    }

    macro_rules! create_creature_card {
        // Base case with additional stats
        ($name:expr, $creature_type:expr, $description:expr, $damage:expr, $defense:expr, [$($mana:expr),*], [$($stat:expr),*] $(, $additional_triggers:expr)*) => {
            {

                let mut card = Card::new(
                    $name,
                    $description,
                    {
                        // Start with the default triggers
                        #[allow(unused_mut)]
                        let mut triggers = vec![
                            // Action to declare the creature as an attacker in the Declare Attackers phase
                            CardActionTrigger::new(
                                ActionTriggerType::AbilityWithinPhases("Attack".to_string(), vec![], Some((vec![TurnPhase::DeclareAttackers], PhaseTarget::Owner)), true),
                                CardRequiredTarget::EnemyCardOrPlayer,
                                Arc::new(DeclareAttackerAction {}),
                            ),
                            // Action to manually declare the creature as a blocker in the Declare Blockers phase
                            CardActionTrigger::new(
                                ActionTriggerType::AbilityWithinPhases("Block".to_string(), vec![], Some((vec![TurnPhase::DeclareBlockers], PhaseTarget::Any)), false),
                                CardRequiredTarget::EnemyCardInCombat,
                                Arc::new(DeclareBlockerAction {}),
                            ),
                        ];

                        // Add any additional triggers provided
                        $(triggers.push($additional_triggers);)*

                        triggers
                    },
                    // Card starts with a charging phase (this can be customized)
                    CardPhase::Charging(1),
                    // Card type is a Creature
                    CardType::Creature,
                    // Add the specified damage, defense, and additional stats
                    {
                        let mut stats = vec![
                            Stat::new(StatType::Power, $damage),
                            Stat::new(StatType::Toughness, $defense),
                        ];

                        // Add any extra stats (e.g. Trample, Flying)
                        $(stats.push(Stat::new($stat, 1));)*

                        stats
                    },
                    // Specify the mana requirements for the creature card
                    vec![$($mana),*],
                );
                card.creature_type = Some($creature_type);
                card
            }


        };
    }

    pub(crate) use create_creature_card;
    pub(crate) use create_multiple_cards;
    pub(crate) use create_planeswalker_card;
}
