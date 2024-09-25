use std::{borrow::BorrowMut, cell::RefCell, rc::Rc, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{player::Player, Game};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TurnPhase {
    Untap,
    Upkeep,
    Draw,
    Main,
    BeginningOfCombat,
    DeclareAttackers,
    DeclareBlockers,
    CombatDamage,
    EndOfCombat,
    End,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Turn {
    #[serde(skip_serializing, skip_deserializing)]
    pub current_player: Arc<Mutex<Player>>,
    pub current_player_index: usize,
    pub phase: TurnPhase,
    pub turn_number: usize,
}

impl Turn {
    pub fn new(
        current_player: Arc<Mutex<Player>>,
        current_player_index: usize,
        turn_number: usize,
    ) -> Self {
        Self {
            current_player,
            current_player_index,
            phase: TurnPhase::Untap, // Start the turn in the Untap phase
            turn_number,
        }
    }

    pub fn next_phase(&mut self) {
        self.phase = match self.phase {
            TurnPhase::Untap => TurnPhase::Upkeep,
            TurnPhase::Upkeep => TurnPhase::Draw,
            TurnPhase::Draw => TurnPhase::Main,
            TurnPhase::Main => TurnPhase::BeginningOfCombat,
            TurnPhase::BeginningOfCombat => TurnPhase::DeclareAttackers,
            TurnPhase::DeclareAttackers => TurnPhase::DeclareBlockers,
            TurnPhase::DeclareBlockers => TurnPhase::CombatDamage,
            TurnPhase::CombatDamage => TurnPhase::EndOfCombat,
            TurnPhase::EndOfCombat => TurnPhase::End,
            TurnPhase::End => TurnPhase::Untap,
        };
    }
}
