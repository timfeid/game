use std::fmt::Debug;
use std::sync::Arc;
use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Mutex;
use ulid::Ulid;
use uuid::uuid;

#[async_trait::async_trait]
pub trait CardStatChangeListener: Debug + Send + Sync {
    async fn on_stat_change(&self, stat_manager: &StatManager);
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Stat {
    pub stat_type: StatType,
    pub intensity: i16,
}
#[async_trait::async_trait]
pub trait Stats: Debug + Send + Sync {
    fn add_stat(&mut self, id: String, stat: Stat);
    fn remove_stat(&mut self, id: String);
    fn get_stat_value(&self, stat_type: StatType) -> i16;
    fn modify_stat(&mut self, stat_type: StatType, intensity: i16);
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, Type)]
pub struct StatManager {
    pub stats: HashMap<String, Stat>,
    #[serde(skip_serializing, skip_deserializing)]
    pub listeners: Vec<Arc<Box<dyn CardStatChangeListener + Send + Sync>>>, // Use Arc<Mutex> for shared ownership
}

#[derive(Type, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatType {
    Health,
    Power,
    LuckToken,
    Toughness,
    Trample,
    Lifelink,
    Flying,
    Reach,
    Regenerate,
    Deathtouch,
    Vigilance,
    Counter,
    Haist,
}

#[derive(Debug)]
pub enum StaticStatId {
    Regenerate,
    Flying,
    Lifelink,
    Trample,
}
impl fmt::Display for StaticStatId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Stat {
    pub fn new(stat_type: StatType, intensity: i16) -> Stat {
        Stat {
            stat_type,
            intensity,
        }
    }

    pub fn set_intensity(&mut self, intensity: i16) {
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

#[async_trait::async_trait]
impl Stats for StatManager {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stats.insert(id, stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i16 {
        let values = &self.stats;
        values
            .iter()
            .filter(|(_id, s)| s.stat_type == stat_type)
            .map(|(_id, s)| s.intensity)
            .sum()
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i16) {
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
            listeners: vec![],
        };

        for stat in stats {
            s.stats.insert(Ulid::new().to_string(), stat);
        }

        s
    }

    pub fn add_listener(&mut self, listener: Arc<Box<dyn CardStatChangeListener + Send + Sync>>) {
        self.listeners.push(listener);
    }
}
