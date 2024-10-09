pub mod black;
pub mod blue;
pub mod green;
pub mod red;
pub mod white;

use rand::seq::SliceRandom;
use rand::thread_rng;
use std::borrow::BorrowMut;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::vec::Vec;
use tokio::sync::Mutex;
use ulid::Ulid;

use crate::game::action::generate_mana::GenerateManaAction;
use crate::game::action::{
    ActionTriggerType, AsyncClosureAction, AsyncClosureWithCardAction, CardActionTarget,
    CardActionTrigger, CardRequiredTarget, CounterSpellAction, DrawCardAction, PlayerActionTarget,
    ReturnToHandAction, TriggerTarget,
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

use super::player::Player;

#[derive(Debug, Default)]
pub struct Deck {
    pub draw_pile: Vec<Arc<Mutex<Card>>>,
    pub discard_pile: Vec<Arc<Mutex<Card>>>,
    pub destroyed_pile: Vec<Arc<Mutex<Card>>>,
    pub in_game: Vec<Arc<Mutex<Card>>>,
}

fn duplicate_card(base_card: Card, count: usize) -> Vec<Card> {
    let mut cards = Vec::new();
    for i in 0..count {
        let mut card = base_card.clone();
        card.id = format!("{}-{}-{}", card.name, i, card.id);
        cards.push(card);
    }
    cards
}

impl Deck {
    pub fn new(cards: Vec<Card>) -> Self {
        Self {
            draw_pile: cards.into_iter().map(|c| Arc::new(Mutex::new(c))).collect(),
            discard_pile: vec![],
            destroyed_pile: vec![],
            in_game: vec![],
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

    // Discard a card
    pub fn discard(&mut self, card: Arc<Mutex<Card>>) {
        self.discard_pile.push(card);
    }

    // Destroy a card
    pub fn destroy(&mut self, card: Arc<Mutex<Card>>) {
        self.destroyed_pile.push(card);
    }

    pub fn elsewhere(&mut self, card: Arc<Mutex<Card>>) {
        self.in_game.push(card);
    }

    // Reshuffle discard pile back into the draw pile
    fn reshuffle_discard_pile(&mut self) {
        self.draw_pile.append(&mut self.discard_pile);
        self.shuffle();
    }

    pub async fn set_owner(&self, player: &Arc<Mutex<Player>>) {
        for card in self.draw_pile.iter() {
            let mut d = card.lock().await;
            d.owner = Some(Arc::clone(player));
        }
    }
}
