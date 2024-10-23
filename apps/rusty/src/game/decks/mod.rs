// pub mod black;
// pub mod blue;
// pub mod green;
// pub mod green_a;
// pub mod red;
pub mod white;

use rand::seq::SliceRandom;
use rand::thread_rng;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::vec::Vec;
use tokio::sync::Mutex;
use ulid::Ulid;
use white::{create_angels_blue_deck, create_angels_deck};

use crate::game::action::generate_mana::GenerateManaAction;
use crate::game::action::{
    ActionTriggerType, AsyncClosureAction, CardActionTarget, CardActionTrigger, CardRequiredTarget,
    CounterSpellAction, DrawCardAction, PhaseTarget, PlayerActionTarget, ReturnToHandAction,
};
use crate::game::card::card::create_creature_card;
use crate::game::card::{CardPhase, CardType, CreatureType};
use crate::game::mana::ManaType;
use crate::game::stat::{StatType, Stats};

use crate::game::action::{DeclareAttackerAction, DeclareBlockerAction};
use crate::game::card::Card;
use crate::game::stat::Stat;
use crate::game::turn::TurnPhase;
use crate::game::Game;
use crate::lobby::lobby::DeckSelector;

use super::player::Player;

#[derive(Debug, Default)]
pub struct Deck {
    pub draw_pile: Vec<Arc<Mutex<Card>>>,
    pub discard_pile: Vec<Arc<Mutex<Card>>>,
    pub graveyard: Vec<Arc<Mutex<Card>>>,
    pub in_game: Vec<Arc<Mutex<Card>>>,
    pub exiled: Vec<Arc<Mutex<Card>>>,
    pub map: HashMap<String, Card>,
}

fn duplicate_card(base_card: Card, count: usize) -> Vec<Card> {
    let mut cards = Vec::new();
    for i in 0..count {
        let mut card = base_card.clone();
        card.triggers
            .iter_mut()
            .for_each(|x| x.id = format!("{}-{}-{}", card.name, i, Ulid::new().to_string()));
        card.id = format!("{}-{}-{}", card.name, i, card.id);
        cards.push(card);
    }
    cards
}

impl Deck {
    pub fn cards_from_selection(selection: &DeckSelector) -> Vec<Card> {
        match selection {
            // DeckSelector::Elves => create_green_deck(),
            // DeckSelector::Elves2 => create_green_deck_v2(),
            // DeckSelector::Blue => create_blue_deck(),
            // DeckSelector::Black => create_black_deck(),
            DeckSelector::Angels => create_angels_deck(),
            // DeckSelector::Red => create_red_deck(),
            // DeckSelector::AngelsBlue => create_angels_blue_deck(),
        }
    }
    pub fn new_from_selection(selection: &DeckSelector) -> Self {
        Deck::new(Deck::cards_from_selection(selection))
    }

    pub fn new(cards: Vec<Card>) -> Self {
        let mut map = HashMap::new();
        for card in cards.iter() {
            map.insert(card.id.clone(), card.clone());
        }

        Self {
            draw_pile: cards.into_iter().map(|c| Arc::new(Mutex::new(c))).collect(),
            discard_pile: vec![],
            graveyard: vec![],
            in_game: vec![],
            exiled: vec![],
            map,
        }
    }

    // Shuffle the draw pile
    pub async fn first_shuffle(&mut self) {
        let mut deck_has_lands = false;
        for card in self.draw_pile.iter() {
            if let CardType::BasicLand(_) = card.lock().await.card_type {
                deck_has_lands = true;
                break;
            }
        }

        if deck_has_lands {
            let mut has_land = false;
            while !has_land {
                self.draw_pile.shuffle(&mut thread_rng());
                println!("Shuffled deck");
                for card in self.draw_pile[self.draw_pile.len() - 7..].iter() {
                    match card.lock().await.card_type {
                        CardType::BasicLand(_) => {
                            has_land = true;
                        }
                        CardType::AdvancedLand(_) => has_land = true,
                        CardType::AdvancedMultiLand(_, _) => has_land = true,
                        _ => {}
                    }
                }
            }
        }
    }

    // Shuffle the draw pile
    pub fn shuffle(&mut self) {
        self.draw_pile.shuffle(&mut thread_rng());
    }

    // Draw a card from the draw pile, or shuffle the discard pile back in
    pub fn draw(&mut self) -> Option<Arc<Mutex<Card>>> {
        if let Some(card) = self.draw_pile.pop() {
            self.in_game.push(card.clone()); // Add to in-game pile
            Some(card)
        } else {
            None // No more cards to draw
        }
    }

    // Destroy a card
    pub fn destroy(&mut self, id: String) {
        if let Some(card) = self.fresh_ref(id) {
            self.graveyard.push(card);
        }
    }

    pub fn elsewhere(&mut self, card: Arc<Mutex<Card>>) {
        self.in_game.push(card);
    }

    pub async fn set_owner(&mut self, player: &Arc<Mutex<Player>>) {
        for card in self.map.values_mut() {
            card.owner = Some(Arc::clone(player));
        }
        for card in self.draw_pile.iter() {
            let mut d = card.lock().await;
            d.owner = Some(Arc::clone(player));
        }
    }

    pub fn exile(&mut self, id: String) {
        if let Some(card) = self.fresh_ref(id) {
            self.exiled.push(card);
        }
    }

    pub(crate) fn fresh(&self, id: String) -> Option<Card> {
        if let Some(x) = self.map.get(id.as_str()) {
            return Some(x.clone());
        }

        None
    }

    pub(crate) fn fresh_ref(&self, id: String) -> Option<Arc<Mutex<Card>>> {
        if let Some(x) = self.map.get(id.as_str()) {
            return Some(Arc::new(Mutex::new(x.clone())));
        }

        None
    }
}
