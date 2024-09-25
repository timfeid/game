use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;
use std::{borrow::Borrow, cell::RefCell};

use serde::{Deserialize, Serialize};
use specta::Type;
use textwrap::fill;
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::game::action;
use crate::game::effects::{EffectManager, EffectTarget};

use super::action::{
    ActionTriggerType, Attachable, CardAction, CardActionTrigger, CardActionWrapper, PlayerAction,
    PlayerActionTrigger, ResetCardAction,
};
use super::effects::EffectID;
use super::mana::ManaType;
use super::turn::Turn;
use super::{
    action::Action,
    player::Player,
    stat::{Stat, StatManager, StatType, Stats},
    turn::TurnPhase,
    Game,
};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Type)]
pub enum CardType {
    Creature,
    Enchantment,
    Equipment,
    Instant,
    Trap,
    Artifact,
    Mana,
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
    pub name: String,
    pub description: String,
    pub card_type: CardType,
    pub current_phase: CardPhase,
    #[serde(skip_serializing, skip_deserializing)]
    pub target: Option<EffectTarget>,
    pub tapped: bool,
    #[serde(skip_serializing, skip_deserializing)]
    pub stats: StatManager,
    #[serde(skip_serializing, skip_deserializing)]
    pub triggers: Vec<CardActionTrigger>,
    pub cost: Vec<ManaType>,
    #[serde(skip_serializing, skip_deserializing)]
    pub owner: Option<Arc<Mutex<Player>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub effect_ids: Vec<EffectID>,
    #[serde(skip_serializing, skip_deserializing)]
    pub attached: Option<Arc<Mutex<Card>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub action_target: Option<EffectTarget>,
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
            effect_ids: vec![],
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
        };
        card.triggers.push(CardActionTrigger::new(
            ActionTriggerType::OnCardDestroyed,
            Arc::new(ResetCardAction {}),
        ));

        card
    }

    pub fn collect_phase_based_actions_sync(
        &self,
        turn: &Turn,
        trigger_type: ActionTriggerType,
        card_arc: &Arc<Mutex<Card>>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut phase_based_actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        let owner = match &self.owner {
            Some(owner) => Arc::clone(owner),
            None => return phase_based_actions,
        };

        for action_trigger in &self.triggers {
            if let ActionTriggerType::PhaseBased(trigger_phase, trigger_target) =
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
                    }));
                }
            } else if &trigger_type == &action_trigger.trigger_type {
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
                }));
            }
        }

        phase_based_actions
    }

    pub async fn collect_manual_actions(
        &self,
        card_arc: Arc<Mutex<Card>>,
        turn_phase: TurnPhase,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for action_trigger in &self.triggers {
            match &action_trigger.trigger_type {
                ActionTriggerType::Tap => {
                    actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(&card_arc),
                        action: action_trigger.action.clone(),
                    }));
                }
                ActionTriggerType::ManualWithinPhases(mana_requirements, allowed_phases) => {
                    if allowed_phases.contains(&turn_phase) {
                        actions.push(Arc::new(CardActionWrapper {
                            card: Arc::clone(&card_arc),
                            action: action_trigger.action.clone(),
                        }));
                    }
                }
                ActionTriggerType::TapWithinPhases(allowed_phases) => {
                    if allowed_phases.contains(&turn_phase) {
                        actions.push(Arc::new(CardActionWrapper {
                            card: Arc::clone(&card_arc),
                            action: action_trigger.action.clone(),
                        }));
                    }
                }
                _ => {}
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
            if let ActionTriggerType::PhaseBased(trigger_phase, trigger_target) =
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
                    }));
                }
            } else if &trigger_type == &action_trigger.trigger_type {
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(card_arc),
                    action: action_trigger.action.clone(),
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
        if self.tapped {
            return Err("Card already tapped");
        }

        println!("{} was tapped", self.name);

        self.tapped = true;

        Ok(())
    }

    async fn collect_attached_actions(
        card: &Arc<Mutex<Card>>,
        target_card: &Arc<Mutex<Card>>,
        game: &mut Game,
    ) -> Option<Vec<Arc<dyn Action + Send + Sync>>> {
        let attached_actions = Card::collect_phase_based_actions(
            card,
            &game.current_turn.clone().unwrap(),
            ActionTriggerType::Attached,
        )
        .await;

        if attached_actions.is_empty() {
            None
        } else {
            Some(attached_actions)
        }
    }

    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!("┌{}┐", "─".repeat(width - 2)));
        lines.push(format!("│{:^width$}│", self.name, width = width - 2));
        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let mana_costs_content = self.format_mana_cost();
        let stats = format!(
            "{}/{}",
            self.get_stat_value(StatType::Damage),
            self.get_stat_value(StatType::Defense),
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
    macro_rules! create_creature_card {
        ($name:expr, $description:expr, $damage:expr, $defense:expr, $mana:expr) => {
            Card::new(
                $name,
                $description,
                vec![
                    // Action to declare the creature as an attacker in the Declare Attackers phase
                    CardActionTrigger::new(
                        ActionTriggerType::TapWithinPhases(vec![TurnPhase::DeclareAttackers]),
                        Arc::new(DeclareAttackerAction {}),
                    ),
                    // Action to manually declare the creature as a blocker in the Declare Blockers phase
                    CardActionTrigger::new(
                        ActionTriggerType::ManualWithinPhases(vec![], vec![TurnPhase::DeclareBlockers]),
                        Arc::new(DeclareBlockerAction {}),
                    ),
                ],
                // Card starts with a charging phase (this can be customized)
                CardPhase::Charging(1),
                // Card type is a Creature
                CardType::Creature,
                // Add the specified damage and defense stats
                vec![
                    Stat::new(StatType::Damage, $damage),
                    Stat::new(StatType::Defense, $defense),
                ],
                // Specify the mana requirements for the creature card
                vec![$mana],
            )
        };
    }

    pub(crate) use create_creature_card;
}
