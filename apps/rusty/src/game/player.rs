use std::{
    borrow::{Borrow, BorrowMut},
    cell::{RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use axum::response::sse::KeepAlive;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use textwrap::fill;
use tokio::{
    sync::Mutex,
    time::{sleep, timeout},
};
use ulid::Ulid;

use crate::{
    error::{AppError, AppResult},
    game::{action::DestroyTargetCAction, card::CardType, mana::ManaType, turn},
};

use super::{
    action::{
        generate_mana::GenerateManaAction, Action, ActionTriggerType, Attachable,
        CardActionTrigger, CardActionWrapper, CombatAction, DrawCardAction, PhaseTarget,
        PlayCardAction, PlayerAction, PlayerActionTarget, PlayerActionTrigger, PlayerActionWrapper,
        ResetManaPoolAction, UntapAllAction,
    },
    card::{Card, CardPhase, CreatureType},
    decks::Deck,
    effects::{Effect, EffectID, EffectManager, EffectTarget},
    mana::ManaPool,
    stat::{Stat, StatManager, StatType, Stats},
    turn::{Turn, TurnPhase},
    FrontendTarget, Game,
};

use std::hash::{Hash, Hasher};

#[derive(Clone)]
pub struct PlayerKey(pub Arc<Mutex<Player>>);

impl PartialEq for PlayerKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for PlayerKey {}

impl Hash for PlayerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ptr = Arc::as_ptr(&self.0);
        ptr.hash(state);
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct Player {
    pub name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub stat_manager: StatManager,
    pub is_alive: bool,
    #[serde(skip_serializing, skip_deserializing)]
    pub deck: Deck,
    #[serde(skip_serializing, skip_deserializing)]
    pub cards_in_hand: Vec<Arc<Mutex<Card>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub cards_in_play: Vec<Arc<Mutex<Card>>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub triggers: Vec<PlayerActionTrigger>,
    #[serde(skip_serializing, skip_deserializing)]
    rendered_output: String, // Store the rendered output here
    pub mana_pool: ManaPool,
    #[serde(skip_serializing, skip_deserializing)]
    pub effect_ids: Vec<EffectID>, // IDs of effects applied to this card
    #[serde(skip_serializing, skip_deserializing)]
    pub game: Option<Arc<Mutex<Game>>>, // Add this field
    #[serde(skip_serializing, skip_deserializing)]
    pub health_at_start_of_round: i16,
    #[serde(skip_serializing, skip_deserializing)]
    pub spells: Vec<Arc<Mutex<Card>>>,
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Just print the stored rendered output
        write!(f, "{}", self.rendered_output)
    }
}

impl Player {
    pub fn reset_spells(&mut self) {
        self.spells = vec![];
    }

    pub async fn add_stat_type(&mut self, stat_type: StatType, amount: i16) {
        self.stat_manager
            .add_stat(Ulid::new().to_string(), Stat::new(stat_type, amount));
    }

    pub fn new(name: &str, health: i16, deck: Vec<Card>) -> Self {
        let mut player = Self {
            name: name.to_string(),
            stat_manager: StatManager::new(vec![Stat::new(StatType::Health, health)]),
            is_alive: true,
            cards_in_hand: vec![],
            health_at_start_of_round: health.clone(),
            cards_in_play: vec![],
            game: None,
            deck: Deck::new(deck),
            spells: vec![],
            triggers: vec![
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseStarted(vec![TurnPhase::Untap], PhaseTarget::Owner),
                    Arc::new(UntapAllAction {}),
                ),
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseStarted(
                        vec![
                            TurnPhase::Untap,
                            TurnPhase::Upkeep,
                            TurnPhase::Draw,
                            TurnPhase::Main,
                            TurnPhase::BeginningOfCombat,
                            TurnPhase::DeclareAttackers,
                            TurnPhase::DeclareBlockers,
                            TurnPhase::CombatDamage,
                            TurnPhase::EndOfCombat,
                            TurnPhase::Main2,
                            TurnPhase::End,
                            TurnPhase::Cleanup,
                        ],
                        PhaseTarget::Owner,
                    ),
                    Arc::new(ResetManaPoolAction {}),
                ),
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseStarted(vec![TurnPhase::Draw], PhaseTarget::Owner),
                    Arc::new(DrawCardAction {}),
                ),
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseStarted(
                        vec![TurnPhase::CombatDamage],
                        PhaseTarget::Owner,
                    ),
                    Arc::new(CombatAction {}),
                ),
            ],
            rendered_output: "".to_string(),
            mana_pool: ManaPool::new(),
            effect_ids: vec![],
        };

        player
    }

    pub fn set_deck(&mut self, card: Vec<Card>) {}

    pub async fn return_card_to_hand(&mut self, card_arc: &Arc<Mutex<Card>>) {
        self.cards_in_play.retain(|c| !Arc::ptr_eq(c, card_arc));
        if let Some(card) = self.deck.fresh_ref(card_arc.lock().await.id.clone()) {
            self.cards_in_hand.push(card);
        }
    }

    // pub async fn

    // pub async fn detach_card_in_play(
    //     &mut self,
    //     index: usize,
    //     game: &mut Game,
    // ) -> Vec<Arc<dyn Action + Send + Sync>> {
    //     let mut actions = vec![];
    //     let mut cards_to_detach = vec![];

    //     if index >= self.cards_in_play.len() {
    //         // Index out of bounds, return empty actions
    //         return actions;
    //     }

    //     let card_arc = self.cards_in_play[index].clone();
    //     cards_to_detach.push(card_arc.clone());

    //     while let Some(card_arc) = cards_to_detach.pop() {
    //         // Remove effects associated with this card
    //         game.effect_manager
    //             .remove_effects_by_source(&card_arc)
    //             .await;

    //         // Lock the card to access its fields
    //         let mut card = card_arc.lock().await;
    //         println!("Removing effects from source card {}", card.name);

    //         // If this card has an attached card, detach it and add to cards_to_detach
    //         if let Some(attached_arc) = card.attached.take() {
    //             cards_to_detach.push(attached_arc);
    //         }

    //         // Release the lock on card before proceeding
    //         drop(card);

    //         // Remove references to this card from other cards
    //         for other_card_arc in &self.cards_in_play {
    //             let mut other_card = other_card_arc.lock().await;

    //             // Remove references from target
    //             // if let Some(t) = &other_card.target {
    //             //     if let EffectTarget::Card(arc) = &t {
    //             //         if Arc::ptr_eq(arc, &card_arc) {
    //             //             other_card.target = None;
    //             //         }
    //             //     }
    //             // }

    //             // Remove references from action_target
    //             // if let Some(t) = &other_card.action_target {
    //             //     if let EffectTarget::Card(arc) = &t {
    //             //         if Arc::ptr_eq(arc, &card_arc) {
    //             //             other_card.action_target = None;
    //             //         }
    //             //     }
    //             // }

    //             // Remove references from attached
    //             if let Some(attached_arc) = &other_card.attached {
    //                 if Arc::ptr_eq(attached_arc, &card_arc) {
    //                     // If the attached card is the detached card, detach it
    //                     let detached_attached_arc = other_card.attached.take();
    //                     if let Some(detached_attached_arc) = detached_attached_arc {
    //                         cards_to_detach.push(detached_attached_arc);
    //                     }
    //                 }
    //             }
    //         }

    //         // Also, remove the card from cards_in_play if it's still there
    //         // if let Some(pos) = self
    //         //     .cards_in_play
    //         //     .iter()
    //         //     .position(|c| Arc::ptr_eq(c, &card_arc))
    //         // {
    //         //     self.cards_in_play.remove(pos);
    //         // }

    //         // Optionally, accumulate any actions resulting from detaching the card
    //         // actions.extend(card.on_detach_actions());
    //     }

    //     for card in cards_to_detach {
    //         if !Arc::ptr_eq(&card, &card_arc) {
    //             actions.push(Arc::new(DetachCardAction::new(&card)));
    //         }
    //     }

    //     actions
    // }

    // pub async fn apply_effects_for_phase(&self, phase: TurnPhase, effect_manager: &EffectManager) {
    //     // Logic to apply player-specific effects for a given phase
    //     for effect_id in &self.effect_ids {
    //         if let Some(effect) = effect_manager.get_effect_by_id(effect_id) {
    //             if effect.applies_in_phase(phase) {
    //                 effect.apply(&EffectTarget::Player(Arc::new(Mutex::new(self))));
    //             }
    //         }
    //     }
    // }

    pub async fn render_cards(
        &self,
        cards: &Vec<Arc<Mutex<Card>>>,
        card_width: usize,
    ) -> Vec<String> {
        let card_height = 10;
        let mut lines = vec![String::new(); card_height];

        // Collect the rendering futures for each card
        let render_futures = cards
            .iter()
            .map(|card| {
                let card = Arc::clone(card);
                async move { card.lock().await.render(card_width) }
            })
            .collect::<Vec<_>>();

        // Wait for all renderings to finish
        let rendered_cards: Vec<Vec<String>> = join_all(render_futures).await;

        // Combine the rendered cards into the final lines
        for i in 0..card_height {
            for (j, card) in rendered_cards.iter().enumerate() {
                if let Some(line) = card.get(i) {
                    // Ensure that each card line fits within card_width
                    lines[i].push_str(&format!("{:<width$}", line, width = card_width));
                } else {
                    // If the card has fewer lines, fill the space with empty lines
                    lines[i].push_str(&" ".repeat(card_width));
                }

                // Add spacing between cards except for the last one
                if j < rendered_cards.len() - 1 {
                    lines[i].push_str("  "); // Two spaces between cards
                }
            }
        }

        lines
    }

    // Renders player information (e.g., stats, name, status) for display
    fn render_player_info(&self, height: usize, width: usize) -> Vec<String> {
        let mut lines = Vec::new();

        lines.push(format!("┌{}┐", "─".repeat(width - 2)));

        lines.push(format!("│{:^width$}│", self.name, width = width - 2));

        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let label = " Alive: ";
        let value_width = width - 2 - label.len();
        let status_line = format!(
            "│{}{: <value_width$}│",
            label,
            self.is_alive,
            value_width = value_width
        );
        lines.push(status_line);

        lines.push(format!("├{}┤", "─".repeat(width - 2)));

        let mut stat_map: HashMap<StatType, i16> = HashMap::new();
        for (_, stat) in &self.stat_manager.stats {
            *stat_map.entry(stat.stat_type.clone()).or_insert(0) += stat.intensity;
        }

        for (stat_type, intensity) in stat_map {
            let stat_line = format!("{}: {}", stat_type, intensity);
            lines.push(format!("│ {: <width$}│", stat_line, width = width - 3));
        }
        let stat_line = format!("Mana: {}", self.mana_pool.format_mana());
        lines.push(format!("│ {: <width$}│", stat_line, width = width - 3));

        // Fill remaining lines with empty space to match the height
        while lines.len() < height - 1 {
            lines.push(format!("│{: <width$}│", " ", width = width - 2));
        }

        lines.push(format!("└{}┘", "─".repeat(width - 2)));

        lines
    }

    pub async fn advance_card_phases(&mut self) {
        println!("advancing card phases for {}", self.name);
        // Collect indices and card arcs to avoid holding locks across awaits
        let card_arcs: Vec<Arc<Mutex<Card>>> = self.cards_in_play.clone();

        for card_arc in card_arcs {
            // Advance the card's phase
            let mut card = card_arc.lock().await;

            match &mut card.current_phase {
                CardPhase::Charging(turns_remaining) => {
                    if *turns_remaining > 0 {
                        *turns_remaining -= 1;
                        if *turns_remaining == 0 {
                            card.current_phase = CardPhase::Ready;
                            println!("Card '{}' is now Ready.", card.name);
                        }
                    }
                }
                // Handle other phases if necessary
                _ => {}
            }
        }
    }

    pub async fn priority_turn_start(&self) {
        println!("{}'s priority turn starts.", self.name);
    }

    pub async fn priority_turn_end(&self) {
        println!("{}'s priority turn ends.", self.name);
    }

    pub async fn filter_cards_in_play<F>(&self, closure: F) -> Vec<Arc<Mutex<Card>>>
    where
        F: Fn(&Card) -> bool + 'static + Send + Sync,
    {
        let mut cards = vec![];
        for card_arc in &self.cards_in_play {
            let card = card_arc.lock().await;
            if (closure)(&card) {
                cards.push(card_arc.clone());
            }
        }

        cards
    }

    // pub async fn execute_action(
    //     &mut self,
    //     in_play_index: usize,
    //     target: Option<EffectTarget>,
    //     game: Arc<Mutex<Game>>,
    //     trigger_id: String,
    // ) -> Result<Vec<Arc<dyn Action + Send + Sync>>, String> {
    //     let actions = {
    //         let card = Arc::clone(&player.lock().await.cards_in_play[in_play_index]);
    //         let phase = game.lock().await.current_turn.as_ref().unwrap().phase;
    //         // card_l.action_target = target.clone();

    //         let (actions, requires_tap, mana_requirements) = Card::collect_manual_actions(
    //             card.clone(),
    //             phase,
    //             target.clone(),
    //             trigger_id.clone(),
    //             game.clone(),
    //         )
    //         .await;

    //         if requires_tap {
    //             card.lock().await.tap()?;
    //         }

    //         player.lock().await.pay_mana(&mana_requirements).await?;

    //         actions
    //     };
    //     println!("{:?}", actions);

    //     Ok(actions)
    // }

    // pub async fn tap_card(
    //     &mut self,
    //     index: usize,
    //     target: Option<EffectTarget>,
    //     game: &mut Game,
    // ) -> Result<Vec<Arc<dyn Action + Send + Sync>>, String> {
    //     let card = &self.cards_in_play[index];
    //     let mutex = card.clone();
    //     let mut card_l = mutex.lock().await;

    //     card_l.tap()?;
    // }
    //     // let actions = Card::collect_phase_based_actions(
    //     //     &card,
    //     //     game.current_turn.as_ref().unwrap(),
    //     //     ActionTriggerType::Manual,
    //     // )
    //     // .await;

    //     let response = card_l
    //         .collect_manual_actions(
    //             Arc::clone(card),
    //             game.current_turn.as_ref().unwrap().phase,
    //             target,
    //             trigger
    //         )
    //         .await;

    //     Ok(response.0)
    // }

    // pub async fn can_pay_mana(&self, mana: Vec<ManaType>) -> bool {
    //     let mut required_mana = ManaPool::new();

    //     for mana_type in &mana {
    //         required_mana.add_mana(mana_type.clone());
    //     }

    //     let mut available_lands = vec![];
    //     for card in &self.cards_in_play {
    //         let card = card.lock().await;
    //         if let CardType::BasicLand(mana_type) /*| CardType::DualLand(mana_types)*/ = &card.card_type
    //         {
    //             if !card.tapped {
    //                 available_lands.push(card.card_type.clone());
    //             }
    //         }
    //     }

    //     // Check if the available lands can satisfy the required mana
    //     for mana_type in &mana {
    //         let mut found = false;
    //         for (index, available) in available_lands.iter().enumerate() {
    //             match available {
    //                 CardType::BasicLand(mt) if mt == mana_type => {
    //                     available_lands.remove(index);
    //                     found = true;
    //                     break;
    //                 }
    //                 // CardType::DualLand(mana_types) if mana_types.contains(mana_type) => {
    //                 //     available_lands.remove(index);
    //                 //     found = true;
    //                 //     break;
    //                 // }
    //                 _ => {}
    //             }
    //         }
    //         if !found {
    //             return false;
    //         }
    //     }

    //     true
    // }
    pub async fn creatures_of_type(&self, creature_type: CreatureType) -> Vec<Arc<Mutex<Card>>> {
        let mut cards: Vec<Arc<Mutex<Card>>> = vec![];
        for card_arc in &self.cards_in_play {
            let card = card_arc.lock().await;
            if card.creature_type == Some(creature_type) {
                cards.push(Arc::clone(card_arc));
            }
        }

        cards
    }

    pub async fn can_pay_mana(&self, mana: &Vec<ManaType>) -> bool {
        let mut required_mana = ManaPool::new();

        for mana_type in mana {
            required_mana.add_mana(mana_type.clone());
        }

        let mut available_lands = vec![];
        {
            for card_arc in &self.cards_in_play {
                // Attempt to acquire the lock with a 50ms timeout
                match timeout(Duration::from_millis(50), card_arc.lock()).await {
                    Ok(card_guard) => {
                        let card = card_guard; // Successfully acquired the lock
                        if !card.tapped {
                            for trigger in &card.triggers {
                                if let Some(generate_mana_action) =
                                    trigger.action.as_any().downcast_ref::<GenerateManaAction>()
                                {
                                    available_lands.push((
                                        card.card_type.clone(),
                                        generate_mana_action.mana_to_add.clone(),
                                    ));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Timeout occurred, skip this card and continue to the next one
                        continue;
                    }
                }
            }
        }

        // Check if the available lands can satisfy the required mana
        for mana_type in &required_mana.to_vec() {
            let mut found = false;
            for (index, (_, available_mana)) in available_lands.iter_mut().enumerate() {
                if let Some(pos) = available_mana.iter().position(|m| m == mana_type) {
                    available_mana.remove(pos);
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }

        true
    }

    pub fn draw_card(&mut self) -> Option<Arc<Mutex<Card>>> {
        if let Some(card) = self.deck.draw() {
            self.cards_in_hand.push(card.clone());
            Some(card)
        } else {
            None
        }
    }

    pub async fn exile_card_in_play(
        &mut self,
        card_index: usize,
        // game: &mut Game,
    ) {
        let card = &self.cards_in_play.remove(card_index);
        self.deck.exile(card.lock().await.id.clone());
    }

    pub async fn destroy_card_in_play(
        &mut self,
        card_index: usize,
        // game: &mut Game,
    ) {
        let card = &self.cards_in_play.remove(card_index);
        self.deck.destroy(card.lock().await.id.clone());
    }

    // pub async fn collect_available_actions(
    //     &self,
    //     turn: Turn,
    // ) -> Vec<Arc<dyn Action + Send + Sync>> {
    //     let mut actions = Vec::new();
    //     let turn_phase = turn.phase;

    //     // Collect manual actions from cards in play
    //     for card_arc in &self.cards_in_play {
    //         let card = card_arc.lock().await;
    //         let mut card_actions = card
    //             .collect_manual_actions(Arc::clone(card_arc), turn_phase, None)
    //             .await;
    //         actions.append(&mut card_actions.0);
    //     }

    //     // Add other player actions if needed

    //     actions
    // }

    pub async fn render(
        &self,
        card_width: usize,
        card_height: usize,
        player_info_width: usize,
    ) -> String {
        let rendered_cards_in_hand = self.render_card_list(&self.cards_in_hand, card_width).await;
        let rendered_cards_in_play = self.render_card_list(&self.cards_in_play, card_width).await;
        let rendered_player_info = self.render_player_info(card_height, player_info_width);

        let mut output = String::new();
        for i in 0..card_height {
            // Render player info
            if let Some(player_line) = rendered_player_info.get(i) {
                output.push_str(&format!("{}", player_line));
            } else {
                output.push_str(&format!("{:width$}", " ", width = player_info_width));
            }

            output.push_str(" H  ");

            // Render cards in hand
            if let Some(card_line) = rendered_cards_in_hand.get(i) {
                output.push_str(&format!("{}", card_line));
            } else {
                output.push_str(&format!("{:width$}", "none", width = card_width));
            }

            output.push_str(" P  ");

            // Render cards in play
            if let Some(card_line) = rendered_cards_in_play.get(i) {
                output.push_str(&format!("{}", card_line));
            } else {
                output.push_str(&format!("{:width$}", "none", width = card_width));
            }

            output.push('\n');
        }

        output
    }

    pub async fn render_card_list(
        &self,
        cards: &Vec<Arc<Mutex<Card>>>,
        card_width: usize,
    ) -> Vec<String> {
        let card_height = 10;
        let mut lines = vec![String::new(); card_height];

        // Collect the rendering futures for each card
        let render_futures = cards
            .iter()
            .map(|card| {
                let card = Arc::clone(card);
                async move { card.lock().await.render(card_width) }
            })
            .collect::<Vec<_>>();

        // Wait for all renderings to finish
        let rendered_cards: Vec<Vec<String>> = join_all(render_futures).await;

        // Combine the rendered cards into the final string
        for i in 0..card_height {
            for (j, card) in rendered_cards.iter().enumerate() {
                if let Some(line) = card.get(i) {
                    // Ensure that each card line fits within card_width
                    lines[i].push_str(&format!("{:<width$}", line, width = card_width));
                } else {
                    // If the card has fewer lines, fill the space with empty lines
                    lines[i].push_str(&" ".repeat(card_width));
                }

                // Add spacing between cards except for the last one
                if j < rendered_cards.len() - 1 {
                    lines[i].push_str("  "); // Two spaces between cards
                }
            }
        }

        lines
    }

    // Call this method whenever the player's state changes
    pub async fn on_state_change(&mut self) {
        let card_width = 30;
        let card_height = 10;
        let player_info_width = 20;

        // Re-render and store the updated string
        self.render(card_width, card_height, player_info_width)
            .await;
    }

    pub(crate) fn from_claims(user: &crate::services::jwt::Claims, health: i16) -> Player {
        Player::new(&user.sub, health, vec![])
    }

    pub async fn empty_mana_pool(&mut self) {
        self.mana_pool.empty_pool();
    }

    pub fn display_mana_pool(&self) -> String {
        self.mana_pool.format_mana()
    }

    pub fn has_required_mana(&self, requirement: &Vec<ManaType>) -> bool {
        let mut required_mana = ManaPool::new();

        // Accumulate the required mana
        for mana in requirement {
            required_mana.add_mana(*mana);
        }

        // Check if the player has enough of each specific colored mana
        let has_enough_colored_mana = self.mana_pool.purple >= required_mana.purple
            && self.mana_pool.blue >= required_mana.blue
            && self.mana_pool.gold >= required_mana.gold
            && self.mana_pool.red >= required_mana.red
            && self.mana_pool.green >= required_mana.green;

        if !has_enough_colored_mana {
            return false;
        }

        // Calculate the total remaining mana after paying colored costs
        let remaining_mana = (self.mana_pool.purple - required_mana.purple)
            + (self.mana_pool.blue - required_mana.blue)
            + (self.mana_pool.gold - required_mana.gold)
            + (self.mana_pool.red - required_mana.red)
            + (self.mana_pool.green - required_mana.green)
            + self.mana_pool.colorless;

        // Check if the remaining mana is enough to cover the colorless mana cost
        remaining_mana >= required_mana.colorless
    }

    pub async fn can_play(&self, card: &Arc<Mutex<Card>>, is_my_turn: bool) -> bool {
        let card = card.lock().await;

        match card.card_type {
            CardType::BasicLand(mana_type) => self.mana_pool.played_card == false && is_my_turn,
            CardType::AdvancedLand(mana_type) => self.mana_pool.played_card == false && is_my_turn,
            CardType::AdvancedMultiLand(mana_type, mana_type1) => {
                self.mana_pool.played_card == false && is_my_turn
            }
            // TODO - others..
            _ => true,
        }
    }

    pub async fn pool_has_cost_for_card(&self, card: &Arc<Mutex<Card>>) -> bool {
        let card = card.lock().await;

        self.has_required_mana(&card.cost)
    }

    pub async fn pay_mana_for_card(&mut self, card: &Arc<Mutex<Card>>) -> Result<(), String> {
        let cost = { card.lock().await.cost.clone() };
        self.pay_mana(&cost)
    }

    pub fn pay_mana(&mut self, cost: &Vec<ManaType>) -> Result<(), String> {
        // Counts of required mana
        let mut purple_required = 0;
        let mut blue_required = 0;
        let mut gold_required = 0;
        let mut red_required = 0;
        let mut green_required = 0;
        let mut generic_required = 0;

        // Count the required mana costs
        for mana in cost {
            match mana {
                ManaType::Purple => purple_required += 1,
                ManaType::Blue => blue_required += 1,
                ManaType::Gold => gold_required += 1,
                ManaType::Red => red_required += 1,
                ManaType::Green => green_required += 1,
                ManaType::Colorless => generic_required += 1,
            }
        }

        // Check if the player has enough colored mana
        if self.mana_pool.purple < purple_required
            || self.mana_pool.blue < blue_required
            || self.mana_pool.gold < gold_required
            || self.mana_pool.red < red_required
            || self.mana_pool.green < green_required
        {
            // Not enough colored mana
            // Handle error (e.g., return an error or panic)
            return Err("Not enough colored mana to pay the cost.".to_string());
        }

        // Deduct the colored mana costs
        self.mana_pool.purple -= purple_required;
        self.mana_pool.blue -= blue_required;
        self.mana_pool.gold -= gold_required;
        self.mana_pool.red -= red_required;
        self.mana_pool.green -= green_required;

        // Now calculate the total available mana for generic costs
        let total_available_mana = self.mana_pool.purple
            + self.mana_pool.blue
            + self.mana_pool.gold
            + self.mana_pool.red
            + self.mana_pool.green
            + self.mana_pool.colorless;

        // Check if total available mana is enough to pay for generic mana cost
        if total_available_mana < generic_required {
            // Not enough mana
            // Handle error (e.g., return an error or panic)
            return Err("Not enough mana to pay the generic mana cost.".to_string());
        }

        // Now deduct the generic mana cost from the player's mana pools
        let mut remaining_generic = generic_required;

        // Subtract from colorless mana pool first (optional preference)
        let colorless_to_use = std::cmp::min(self.mana_pool.colorless, remaining_generic);
        self.mana_pool.colorless -= colorless_to_use;
        remaining_generic -= colorless_to_use;

        // Then subtract from colored mana pools
        if remaining_generic > 0 {
            let purple_to_use = std::cmp::min(self.mana_pool.purple, remaining_generic);
            self.mana_pool.purple -= purple_to_use;
            remaining_generic -= purple_to_use;
        }

        if remaining_generic > 0 {
            let blue_to_use = std::cmp::min(self.mana_pool.blue, remaining_generic);
            self.mana_pool.blue -= blue_to_use;
            remaining_generic -= blue_to_use;
        }

        if remaining_generic > 0 {
            let gold_to_use = std::cmp::min(self.mana_pool.gold, remaining_generic);
            self.mana_pool.gold -= gold_to_use;
            remaining_generic -= gold_to_use;
        }

        if remaining_generic > 0 {
            let red_to_use = std::cmp::min(self.mana_pool.red, remaining_generic);
            self.mana_pool.red -= red_to_use;
            remaining_generic -= red_to_use;
        }

        if remaining_generic > 0 {
            let green_to_use = std::cmp::min(self.mana_pool.green, remaining_generic);
            self.mana_pool.green -= green_to_use;
            remaining_generic -= green_to_use;
        }

        // At this point, remaining_generic should be zero
        if remaining_generic > 0 {
            // This should not happen since we've already checked if we have enough mana
            return Err("Unexpected error: Not all generic mana cost was paid.".to_string());
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Stats for Player {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stat_manager.add_stat(id, stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i16 {
        self.stat_manager.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i16) {
        self.stat_manager.modify_stat(stat_type, intensity);
    }

    fn remove_stat(&mut self, id: String) {
        self.stat_manager.remove_stat(id);
    }
}

impl fmt::Debug for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Player")
            .field("name", &self.name)
            .field("is_alive", &self.is_alive)
            .field(
                "cards_in_hand",
                &format_args!("<{} cards>", self.cards_in_hand.len()),
            )
            .finish()
    }
}
