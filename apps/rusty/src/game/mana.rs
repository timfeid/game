use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Type, Copy)]
pub enum ManaType {
    White,     // {W}
    Blue,      // {U}
    Black,     // {B}
    Red,       // {R}
    Green,     // {G}
    Colorless, // {C}
}

impl ManaType {
    pub fn format(&self) -> String {
        match self {
            ManaType::White => "{W}".to_string(),
            ManaType::Blue => "{U}".to_string(),
            ManaType::Black => "{B}".to_string(),
            ManaType::Red => "{R}".to_string(),
            ManaType::Green => "{G}".to_string(),
            ManaType::Colorless => "{C}".to_string(),
        }
    }
}

#[derive(Type, Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManaPool {
    pub white: u8,
    pub blue: u8,
    pub black: u8,
    pub red: u8,
    pub green: u8,
    pub colorless: u8,
    pub played_card: bool,
}

impl ManaPool {
    pub fn new() -> Self {
        ManaPool {
            white: 0,
            blue: 0,
            black: 0,
            red: 0,
            green: 0,
            colorless: 0,
            played_card: false,
        }
    }

    pub fn add_mana(&mut self, mana: ManaType) {
        match mana {
            ManaType::White => self.white += 1,
            ManaType::Blue => self.blue += 1,
            ManaType::Black => self.black += 1,
            ManaType::Red => self.red += 1,
            ManaType::Green => self.green += 1,
            ManaType::Colorless => self.colorless += 1,
        }
    }

    pub fn empty_pool(&mut self) {
        let played_card = self.played_card;
        *self = ManaPool::new();
        self.played_card = played_card;
    }

    pub fn format_mana(&self) -> String {
        let mut mana_str = String::new();
        if self.white > 0 {
            mana_str.push_str(&format!("{{{}}} ", "W".repeat(self.white as usize)));
        }
        if self.blue > 0 {
            mana_str.push_str(&format!("{{{}}} ", "U".repeat(self.blue as usize)));
        }
        if self.black > 0 {
            mana_str.push_str(&format!("{{{}}} ", "B".repeat(self.black as usize)));
        }
        if self.red > 0 {
            mana_str.push_str(&format!("{{{}}} ", "R".repeat(self.red as usize)));
        }
        if self.green > 0 {
            mana_str.push_str(&format!("{{{}}} ", "G".repeat(self.green as usize)));
        }
        if self.colorless > 0 {
            mana_str.push_str(&format!("{{{}C}}", self.colorless));
        }
        mana_str.trim_end().to_string()
    }
}
