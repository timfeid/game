use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;
use std::{borrow::Borrow, cell::RefCell};

use serde::{Deserialize, Serialize};
use specta::Type;
use textwrap::fill;
use tokio::sync::Mutex;
use ulid::Ulid;

use crate::error::AppError;
use crate::game::action;
use crate::game::effects::{EffectManager, EffectTarget};

use super::action::{
    ActionTriggerType, Attachable, CardAction, CardActionTarget, CardActionTrigger,
    CardActionWrapper, CardRequiredTarget, PlayerAction, PlayerActionTrigger, ResetCardAction,
};

use super::effects::EffectID;
use super::mana::ManaType;
use super::player;
use super::turn::Turn;
use super::{
    action::Action,
    player::Player,
    stat::{Stat, StatManager, StatType, Stats},
    turn::TurnPhase,
    Game,
};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Type)]
pub enum CreatureType {
    None,
    Angel,
    Elf,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Type)]
pub enum CardType {
    Creature,
    Enchantment,
    Instant,
    Sorcery,
    Artifact,
    BasicLand(ManaType),
    // Land(Vec<ManaType>),
}

impl CardType {
    pub fn is_spell(&self) -> bool {
        match self {
            CardType::BasicLand(_) => false,
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

#[derive(Type, Debug, Deserialize, Serialize, Clone)]
pub struct Card {
    pub creature_type: Option<CreatureType>,
    pub name: String,
    pub description: String,
    pub card_type: CardType,
    pub current_phase: CardPhase,
    #[serde(skip_serializing, skip_deserializing)]
    pub target: Option<EffectTarget>,
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
    pub action_target: Option<EffectTarget>,
    #[serde(skip_serializing, skip_deserializing)]
    pub damage_dealt_to_players: i8,
    #[serde(skip_serializing, skip_deserializing)]
    pub damage_taken: i8,
    pub is_countered: bool,
    pub id: String,
}

impl Card {
    pub fn new(
        name: &str,
        description: &str,
        triggers: Vec<CardActionTrigger>,
        phase: CardPhase,
        card_type: CardType,
        stats: Vec<Stat>,
        cost: Vec<ManaType>,
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
            action_target: None,
            damage_taken: 0,
            damage_dealt_to_players: 0,
            is_countered: false,
        };
        card.triggers.push(CardActionTrigger::new(
            ActionTriggerType::CardDestroyed,
            CardRequiredTarget::None,
            Arc::new(ResetCardAction {}),
        ));

        card
    }

    pub fn is_useless(&self, has_effects: bool) -> bool {
        let has_triggers = self
            .triggers
            .iter()
            .filter(|t| match &t.trigger_type {
                ActionTriggerType::CardPlayedFromHand => true,
                ActionTriggerType::AbilityWithinPhases(_, _, _, _) => true,
                ActionTriggerType::PhaseStarted(vec, trigger_target) => true,
                ActionTriggerType::CreatureTypeCardPlayed(trigger_target, creature_type) => true,
                ActionTriggerType::Attached => true,
                ActionTriggerType::DamageApplied => true,
                ActionTriggerType::OtherCardPlayed(_) => true,
                ActionTriggerType::Continuous => false,
                ActionTriggerType::Detached => false,
                ActionTriggerType::CardDestroyed => false,
            })
            .count()
            > 0;

        println!(
            "has_triggers: {:?}\nhas_effects: {}\n\n\n",
            has_triggers, has_effects
        );

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
        target: Option<EffectTarget>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for action_trigger in &self.triggers {
            match &action_trigger.trigger_type {
                ActionTriggerType::Attached => {
                    actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(&card_arc),
                        action: action_trigger.action.clone(),
                        target: target.clone(),
                    }));
                }
                _ => {}
            }
        }

        actions
    }

    pub async fn collect_manual_actions_old(
        &self,
        card_arc: Arc<Mutex<Card>>,
        turn_phase: TurnPhase,
        target: Option<EffectTarget>,
    ) -> (Vec<Arc<dyn Action + Send + Sync>>, bool) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let mut requires_tap = false;

        for action_trigger in &self.triggers {
            match &action_trigger.trigger_type {
                ActionTriggerType::AbilityWithinPhases(
                    _,
                    mana_requirements,
                    allowed_phases,
                    tap_required,
                ) => {
                    let in_phases = allowed_phases.is_none()
                        || allowed_phases.as_ref().unwrap().contains(&turn_phase);

                    if in_phases {
                        requires_tap = tap_required.clone();
                        actions.push(Arc::new(CardActionWrapper {
                            card: Arc::clone(&card_arc),
                            action: action_trigger.action.clone(),
                            target: target.clone(),
                        }));
                    }
                }
                _ => {}
            }
        }

        (actions, requires_tap)
    }

    pub async fn collect_manual_actions(
        card_arc: Arc<Mutex<Card>>,
        turn_phase: TurnPhase,
        target: Option<EffectTarget>,
        trigger_id: String,
        game: Arc<Mutex<Game>>,
    ) -> (Vec<Arc<dyn Action + Send + Sync>>, bool) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let mut requires_tap = false;

        let triggers = { card_arc.lock().await.triggers.clone() };
        for action_trigger in &triggers {
            match &action_trigger.trigger_type {
                ActionTriggerType::AbilityWithinPhases(
                    _,
                    mana_requirements,
                    allowed_phases,
                    tap_required,
                ) => {
                    if trigger_id != action_trigger.id {
                        continue;
                    }
                    println!("we were triggered");
                    let in_phases = allowed_phases.is_none()
                        || allowed_phases.as_ref().unwrap().contains(&turn_phase);

                    println!("getting requirements game: {:?} card: {:?}", game, card_arc);
                    let meets_requirements =
                        (action_trigger.requirements)(Arc::clone(&game), Arc::clone(&card_arc))
                            .await;

                    println!("here is where it matters: {}", meets_requirements);

                    if in_phases && meets_requirements {
                        requires_tap = tap_required.clone();
                        actions.push(Arc::new(CardActionWrapper {
                            card: Arc::clone(&card_arc),
                            action: action_trigger.action.clone(),
                            target: target.clone(),
                        }));
                    }
                }
                _ => {}
            }
        }

        (actions, requires_tap)
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
            if let ActionTriggerType::PhaseStarted(trigger_phase, trigger_target) =
                &action_trigger.trigger_type
            {
                let is_owner = Arc::ptr_eq(&turn.current_player, &owner);
                if trigger_phase.contains(&turn.phase)
                    && match trigger_target {
                        super::action::TriggerTarget::Owner => is_owner,
                        super::action::TriggerTarget::Target => !is_owner,
                        super::action::TriggerTarget::Any => true,
                    }
                {
                    phase_based_actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(card_arc),
                        action: action_trigger.action.clone(),
                        target: match trigger_target {
                            action::TriggerTarget::Owner => {
                                Some(EffectTarget::Player(Arc::clone(&owner)))
                            }
                            action::TriggerTarget::Target => {
                                card_arc.lock().await.action_target.clone()
                            }
                            action::TriggerTarget::Any => None,
                        },
                    }));
                }
            } else if &trigger_type == &action_trigger.trigger_type {
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
                    target: None,
                }));
            } else if action_trigger.trigger_type == ActionTriggerType::CardPlayedFromHand
                && trigger_type != ActionTriggerType::CardDestroyed
            {
                // println!("EXECUTING THIS {}", name);
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
                    target: None,
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

        lines.push(format!("└{}┘", "─".repeat(width - 2)));
        lines
    }

    pub(crate) fn untap(&mut self) {
        self.tapped = false;
    }
}

impl Stats for Card {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stats.add_stat(id, stat);
        println!("{}", self.render(30).join("\n"));
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        self.stats.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
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

    pub(crate) use create_creature_card;
    pub(crate) use create_multiple_cards;
}
