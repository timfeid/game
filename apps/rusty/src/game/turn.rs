use std::{borrow::BorrowMut, cell::RefCell, rc::Rc, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Mutex;

use super::{player::Player, Game};

#[derive(Type, Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
    Main2,
    End,
    Cleanup,
}

#[derive(Type, Debug, Clone, Deserialize, Serialize)]
pub struct Turn {
    #[serde(skip_serializing, skip_deserializing)]
    pub current_player: Arc<Mutex<Player>>,
    pub current_player_index: i32,
    pub phase: TurnPhase,
    pub turn_number: i32,
}

impl Turn {
    pub fn new(
        current_player: Arc<Mutex<Player>>,
        current_player_index: usize,
        turn_number: usize,
    ) -> Self {
        Self {
            current_player,
            current_player_index: current_player_index as i32,
            phase: TurnPhase::Untap, // Start the turn in the Untap phase
            turn_number: turn_number as i32,
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
            TurnPhase::EndOfCombat => TurnPhase::Main2,
            TurnPhase::Main2 => TurnPhase::End,
            TurnPhase::End => TurnPhase::Cleanup,
            TurnPhase::Cleanup => TurnPhase::Untap,
        };
    }
}
