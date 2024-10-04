use std::fmt::Debug;
use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use specta::Type;
use ulid::Ulid;
use uuid::uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Stat {
    pub stat_type: StatType,
    pub intensity: i8,
}
pub trait Stats: Debug + Send + Sync {
    fn add_stat(&mut self, id: String, stat: Stat);
    fn remove_stat(&mut self, id: String);
    fn get_stat_value(&self, stat_type: StatType) -> i8;
    fn modify_stat(&mut self, stat_type: StatType, intensity: i8);
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Type)]
pub struct StatManager {
    pub stats: HashMap<String, Stat>,
}

#[derive(Type, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatType {
    Health,
    Damage,
    Defense,
    Trample,
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
            _ => x,
        };

        write!(f, "{}", stat_name)
    }
}

impl Stats for StatManager {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stats.insert(id, stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        let values = &self.stats;
        values
            .iter()
            .filter(|(_id, s)| s.stat_type == stat_type)
            .map(|(_id, s)| s.intensity)
            .sum()
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
        for stat in &mut self.stats.values_mut() {
            if stat.stat_type == stat_type {
                stat.intensity += intensity;
            }
        }
    }

    fn remove_stat(&mut self, id: String) {
        self.stats.remove(&id);
    }
}

impl StatManager {
    pub fn new(stats: Vec<Stat>) -> Self {
        let mut s = Self {
            stats: HashMap::new(),
        };

        for stat in stats {
            s.add_stat(Ulid::new().to_string(), stat);
        }

        s
    }
}
