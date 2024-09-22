use std::fmt;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    pub stat_type: StatType,
    pub intensity: i8,
}
pub trait Stats: Debug + Send + Sync {
    fn add_stat(&mut self, stat: Stat);
    fn get_stat_value(&self, stat_type: StatType) -> i8;
    fn modify_stat(&mut self, stat_type: StatType, intensity: i8);
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
pub struct StatManager {
    pub stats: Vec<Stat>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatType {
    Health,
    Damage,
    Heat,
    ActionPoints,
    Defense,
}

impl Stat {
    pub fn new(stat_type: StatType, intensity: i8) -> Stat {
        Stat {
            stat_type,
            intensity,
        }
    }

    pub fn set_intensity(&mut self, intensity: i8) {
        self.intensity = intensity
    }
}

impl fmt::Display for Stat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.stat_type, self.intensity)
    }
}

impl fmt::Display for StatType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = &format!("{:?}", self);
        let x = n.as_str();
        let stat_name = match self {
            StatType::ActionPoints => "Action Points",
            _ => x,
        };

        write!(f, "{}", stat_name)
    }
}

impl Stats for StatManager {
    fn add_stat(&mut self, stat: Stat) {
        self.stats.push(stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        self.stats
            .iter()
            .filter(|s| s.stat_type == stat_type)
            .map(|s| s.intensity)
            .sum()
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
        for stat in &mut self.stats {
            if stat.stat_type == stat_type {
                stat.intensity += intensity;
            }
        }
    }
}

impl StatManager {
    pub fn new(stats: Vec<Stat>) -> Self {
        Self { stats }
    }
}
