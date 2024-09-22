use std::fmt::Debug;
use std::rc::Rc;
use std::sync::Arc;
use std::{borrow::Borrow, cell::RefCell};

use serde::{Deserialize, Serialize};
use specta::Type;
use textwrap::fill;
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::game::effects::{EffectManager, EffectTarget};

use super::action::{
    ActionTriggerType, CardAction, CardActionTrigger, CardActionWrapper, PlayerAction,
    PlayerActionTrigger,
};
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
    Monster,
    Enchantment,
    Instant,
    Trap,
    Artifact,
    Mana(ManaType),
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
    pub target: Option<EffectTarget>, // Can be a Player or another Card
    #[serde(skip_serializing, skip_deserializing)]
    pub attached_cards: Vec<Arc<Mutex<Card>>>,
    pub tapped: bool,
    #[serde(skip_serializing, skip_deserializing)]
    pub stats: StatManager,
    #[serde(skip_serializing, skip_deserializing)]
    // pub actions: Vec<Arc<Mutex<Box<dyn CardAction + Send + Sync + 'static>>>>, // Actions attached to the card
    pub triggers: Vec<CardActionTrigger>,
    pub cost: Vec<ManaType>,
    #[serde(skip_serializing, skip_deserializing)]
    pub owner: Option<Arc<Mutex<Player>>>,
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
        Self {
            name: name.to_string(),
            description: description.to_string(),
            tapped: false,
            card_type,
            triggers,
            current_phase: phase,
            target: None,
            attached_cards: vec![],
            stats: StatManager::new(stats),
            cost,
            owner: None,
        }
    }

    pub fn add_trigger(&mut self, trigger: CardActionTrigger) {
        self.triggers.push(trigger);
    }

    // Collect phase-based triggers
    pub async fn collect_phase_based_actions(
        card: &Arc<Mutex<Card>>, // The card is now passed as Arc<Mutex<Card>>
        turn: &Turn,
        trigger_type: ActionTriggerType,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut phase_based_actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        let card = Arc::clone(card);
        let card_l = card.lock().await;
        let owner = card_l.owner.as_ref().unwrap();

        for action_trigger in &card_l.triggers {
            if let ActionTriggerType::PhaseBased(trigger_phase, trigger_target) =
                &action_trigger.trigger_type
            {
                let is_owner = Arc::ptr_eq(&turn.current_player, owner);
                if trigger_phase.contains(&turn.phase)
                    && match trigger_target {
                        super::action::TriggerTarget::Owner => is_owner,
                        super::action::TriggerTarget::Target => !is_owner,
                        super::action::TriggerTarget::Any => true,
                    }
                {
                    phase_based_actions.push(Arc::new(CardActionWrapper {
                        card: Arc::clone(&card),
                        action: action_trigger.action.clone(),
                        owner: Arc::clone(&owner),
                    }));
                }
            } else if &trigger_type == &action_trigger.trigger_type {
                phase_based_actions.push(Arc::new(CardActionWrapper {
                    card: Arc::clone(&card),
                    action: action_trigger.action.clone(),
                    owner: Arc::clone(&owner),
                }))
            }
        }

        phase_based_actions
    }

    // Manually trigger actions (e.g., when the card is tapped)
    pub async fn tap(
        card: &Arc<Mutex<Card>>, // The card is passed as Arc<Mutex<Card>>
    ) -> Result<(), &str> {
        let mut card_locked = card.lock().await;

        if card_locked.tapped {
            return Err("Card already tapped");
        }
        println!("{} was tapped", card_locked.name);

        card_locked.tapped = true;

        Ok(())
    }

    // pub async fn trigger_tap(card: &Arc<Mutex<Card>>, game: &mut Game) {
    //     let triggers = {
    //         // Lock the card and clone the triggers, then release the lock
    //         let card_locked = card.lock().await;
    //         card_locked.triggers.clone()
    //     };

    //     // Now iterate over the triggers outside the lock
    //     for trigger in triggers {
    //         if let ActionTriggerType::Manual = trigger.trigger_type {
    //             trigger.trigger(game, Arc::clone(card)).await;
    //         }
    //     }
    // }

    pub fn attach_card(&mut self, card: Arc<Mutex<Card>>) {
        self.attached_cards.push(card);
    }

    pub fn play_on(&mut self, target: EffectTarget) {
        println!("{} is played on {:?}", self.name, &target);
        self.target = Some(target);
    }

    pub fn cancel(&mut self) {
        println!("{} has been cancelled.", self.name);
        self.current_phase = CardPhase::Cancelled;
    }

    pub fn is_active(&self) -> bool {
        // A card is active if it has effects and isn't complete or cancelled
        // !self.effects.is_empty()
        //     && self.current_phase != Phase::Complete
        //     && self.current_phase != Phase::Cancelled
        true
    }

    pub fn format_mana_cost(&self) -> String {
        let mut formatted_mana = String::new();
        let mut colorless_count = 0;

        for mana in &self.cost {
            match mana {
                ManaType::Colorless => {
                    colorless_count += 1;
                }
                _ => {
                    if colorless_count > 0 {
                        formatted_mana.push_str(&format!("{{{}C}} ", colorless_count));
                        colorless_count = 0;
                    }
                    formatted_mana.push_str(&format!("{} ", mana.format()));
                }
            }
        }

        // If there are any leftover colorless mana, add them to the string
        if colorless_count > 0 {
            formatted_mana.push_str(&format!("{{{}C}}", colorless_count));
        }

        formatted_mana.trim_end().to_string() // Trim trailing space
    }

    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();

        // Top border
        lines.push(format!("┌{}┐", "─".repeat(width - 2)));

        // Card name centered
        lines.push(format!("│{:^width$}│", self.name, width = width - 2));

        // Separator
        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let mana_costs_content = self.format_mana_cost();

        // Action Points, Damage, Defense
        let stats_content = format!(
            " {} {:<width$} {}/{} ",
            mana_costs_content,
            " ",
            self.get_stat_value(StatType::Damage),
            self.get_stat_value(StatType::Defense),
            width = width - 14,
        );
        let content_width = width - 2; // -2 for borders
        let stats_line = format!(
            "│{:<content_width$}│",
            stats_content,
            content_width = content_width
        );
        lines.push(stats_line);

        // Separator
        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        // Description wrapped to fit within the card
        let content_width = width - 4; // -2 for borders, -2 for padding
        let description = fill(&self.description, content_width);
        for desc_line in description.lines() {
            let desc_line_formatted = format!(
                "│ {: <content_width$}│",
                desc_line,
                content_width = content_width + 1
            );
            lines.push(desc_line_formatted);
        }

        // Fill the remaining lines if the description is short
        let content_height = 8; // Adjust as needed
        while lines.len() < content_height {
            lines.push(format!("│{: <width$}│", " ", width = width - 2));
        }

        // Bottom border
        lines.push(format!("└{}┘", "─".repeat(width - 2)));

        lines
    }

    pub(crate) fn untap(&mut self) {
        self.tapped = false
    }

    // pub fn add_action(&mut self, action: impl CardAction + Send + Sync + 'static) {
    //     self.actions.push(Arc::new(Mutex::new(Box::new(action))));
    // }
}

impl Stats for Card {
    fn add_stat(&mut self, stat: Stat) {
        self.stats.add_stat(stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        self.stats.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
        self.stats.modify_stat(stat_type, intensity);
    }
}
