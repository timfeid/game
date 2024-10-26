use rand::distributions::{Distribution, WeightedIndex};
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use specta::Type;

pub struct SlotMachine {
    symbols: Vec<&'static str>,
    winning_combinations: Vec<Vec<&'static str>>,
    symbol_weights: Option<Vec<usize>>,
    percent_chance_of_winning: Option<i8>,
}

#[derive(Type, Clone, Deserialize, Serialize, Debug)]
pub struct SlotMachineResult {
    pub reels: Vec<String>,
    pub won: bool,
    pub symbols_used: Vec<String>,
    pub winning_combinations: Vec<Vec<String>>,
    pub result: String,
}

impl SlotMachine {
    /// Creates a percentage-based SlotMachine.
    pub fn new(percent_chance_of_winning: i8) -> Self {
        let percent_chance_of_winning = percent_chance_of_winning.clamp(1, 100);

        let all_symbols = vec!["🍒", "🍋", "🍊", "🍇", "🔔", "💎", "⭐", "💰"];

        let symbols = match percent_chance_of_winning {
            50 => vec!["✅", "❌"],
            _ => {
                // Calculate the number of symbols based on the percent chance to win
                let mut number_of_symbols =
                    (100f32 / percent_chance_of_winning as f32).round() as usize;
                number_of_symbols = number_of_symbols.max(2).min(all_symbols.len());

                // Select the symbols to use
                all_symbols[..number_of_symbols].to_vec()
            }
        };

        // Define winning combinations (all symbols the same)
        let winning_combinations = symbols
            .iter()
            .map(|&symbol| vec![symbol, symbol, symbol])
            .collect();

        Self {
            symbols,
            winning_combinations,
            symbol_weights: None,
            percent_chance_of_winning: Some(percent_chance_of_winning),
        }
    }

    /// Convenience method for 50% chance slot machine.
    pub fn with_50_percent_chance() -> Self {
        Self::new(50)
    }

    /// Convenience method for 33% chance slot machine.
    pub fn with_33_percent_chance() -> Self {
        Self::new(33)
    }

    /// Creates a SlotMachine with custom symbols and winning combinations.
    pub fn with_custom_symbols_and_combinations(
        symbols: Vec<&'static str>,
        winning_combinations: Vec<Vec<&'static str>>,
    ) -> Self {
        Self {
            symbols,
            winning_combinations,
            symbol_weights: None,
            percent_chance_of_winning: None,
        }
    }

    /// Spins the slot machine and returns the result.
    pub fn spin(&self, winning_message: &str, losing_message: &str) -> SlotMachineResult {
        let mut rng = thread_rng();
        let mut reels = Vec::new();

        // Use symbol weights if provided
        if let Some(weights) = &self.symbol_weights {
            let dist = WeightedIndex::new(weights).unwrap();
            for _ in 0..3 {
                let index = dist.sample(&mut rng);
                let symbol = self.symbols[index];
                reels.push(symbol.to_string());
            }
        } else {
            // Spin the reels randomly from the symbols
            for _ in 0..3 {
                let symbol = self.symbols.choose(&mut rng).unwrap();
                reels.push(symbol.to_string());
            }
        }

        let won = if let Some(percent_chance) = self.percent_chance_of_winning {
            // For percentage-based slot machines, determine win based on chance
            let did_win = rng.gen_range(0..100) < percent_chance as usize;
            if did_win {
                // Ensure winning combination
                let winning_symbol = self.symbols.choose(&mut rng).unwrap();
                reels = vec![winning_symbol.to_string(); 3];
                true
            } else {
                // Ensure it's not a winning combination
                while self
                    .winning_combinations
                    .iter()
                    .any(|combo| combo == &reels.iter().map(String::as_str).collect::<Vec<_>>())
                {
                    reels = self
                        .symbols
                        .choose_multiple(&mut rng, 3)
                        .map(|&s| s.to_string())
                        .collect();
                }
                false
            }
        } else {
            // For custom slot machines, check if reels match any winning combination
            self.winning_combinations
                .iter()
                .any(|combo| combo == &reels.iter().map(String::as_str).collect::<Vec<_>>())
        };

        SlotMachineResult {
            reels,
            won,
            result: if won {
                winning_message.to_string()
            } else {
                losing_message.to_string()
            },
            symbols_used: self.symbols.iter().map(|s| s.to_string()).collect(),
            winning_combinations: self
                .winning_combinations
                .iter()
                .map(|combo| combo.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }
}
