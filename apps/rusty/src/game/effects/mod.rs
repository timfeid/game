pub mod modify_stat_effect;
use std::fmt::Debug;

use std::rc::Rc;
use std::{cell::RefCell, fmt, sync::Arc};

use tokio::sync::Mutex;

use super::{
    card::Card,
    player::Player,
    stat::{StatType, Stats},
    turn::{self, TurnPhase},
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
    // Stats(Arc<Mutex<dyn Stats>>),
    Player(Arc<Mutex<Player>>),
    Card(Arc<Mutex<Card>>),
}

// impl EffectTarget {
//     pub fn as_stats(&self) -> &dyn Stats {
//         match self {
//             EffectTarget::Stats(stats) => stats.get_mut(),
//         }
//     }

//     pub fn as_stats_mut(&mut self) -> &mut dyn Stats {
//         match self {
//             EffectTarget::Stats(stats) => stats.as_mut(),
//         }
//     }
// }

pub trait Effect {
    fn apply(&self, target: &EffectTarget);
    fn applies_in_phase(&self, phase: TurnPhase) -> bool;
}

pub struct EffectManager {
    effects: Vec<Box<dyn Effect>>, // Storing multiple effects
}

impl EffectManager {
    pub fn new(effects: Vec<Box<dyn Effect>>) -> Self {
        Self { effects }
    }

    pub fn add_effect(&mut self, effect: Box<dyn Effect>) {
        self.effects.push(effect);
    }

    pub fn get_effects_for_phase(&self, phase: TurnPhase) -> Vec<&dyn Effect> {
        self.effects
            .iter()
            .filter_map(|e| {
                if e.applies_in_phase(phase) {
                    Some(e.as_ref()) // Dereference the Box to get a &dyn Effect
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn has_active_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    pub(crate) fn is_empty(&self) -> bool {
        println!("is empty called");
        false
    }
}

impl fmt::Debug for EffectManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectManager").finish()
    }
}
