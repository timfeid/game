use std::{
    borrow::{Borrow, BorrowMut},
    cell::{RefCell, RefMut},
    collections::HashMap,
    fmt,
    rc::Rc,
    sync::Arc,
};

use axum::response::sse::KeepAlive;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use textwrap::fill;
use tokio::sync::Mutex;

use crate::{
    error::{AppError, AppResult},
    game::{
        action::{DestroySelfAction, DestroyTargetCAction},
        card::CardType,
        mana::ManaType,
        turn,
    },
};

use super::{
    action::{
        Action, ActionTriggerType, Attachable, CardActionTrigger, CardActionWrapper, CombatAction,
        DrawCardAction, PlayerAction, PlayerActionTarget, PlayerActionTrigger, PlayerActionWrapper,
        TriggerTarget, UntapAllAction,
    },
    card::{Card, CardPhase},
    deck::Deck,
    effects::{Effect, EffectID, EffectManager, EffectTarget},
    mana::ManaPool,
    stat::{Stat, StatManager, StatType, Stats},
    turn::{Turn, TurnPhase},
    Game,
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
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Just print the stored rendered output
        write!(f, "{}", self.rendered_output)
    }
}

impl Player {
    pub fn new(name: &str, health: i8, action_points: i8, deck: Vec<Card>) -> Self {
        let mut player = Self {
            name: name.to_string(),
            stat_manager: StatManager::new(vec![
                Stat::new(StatType::Health, health),
                Stat::new(StatType::ActionPoints, action_points),
            ]),
            is_alive: true,
            cards_in_hand: vec![],
            cards_in_play: vec![],
            game: None,
            deck: Deck::new(deck),
            triggers: vec![
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseBased(vec![TurnPhase::Untap], TriggerTarget::Owner),
                    Arc::new(UntapAllAction {}),
                ),
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseBased(vec![TurnPhase::Draw], TriggerTarget::Owner),
                    Arc::new(DrawCardAction {
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                ),
                PlayerActionTrigger::new(
                    ActionTriggerType::PhaseBased(
                        vec![TurnPhase::CombatDamage],
                        TriggerTarget::Owner,
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

    pub async fn detach_card_in_play(
        &mut self,
        index: usize,
        game: &mut Game,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions = vec![];
        let mut indices_to_detach = vec![index];

        while let Some(current_index) = indices_to_detach.pop() {
            if current_index >= self.cards_in_play.len() {
                continue; // Index out of bounds, skip
            }

            let card_arc = self.cards_in_play.remove(current_index);

            // Remove effects applied by this card
            game.effect_manager
                .remove_effects_by_source(&card_arc)
                .await;

            // Handle attached cards
            if let Some(attached_arc) = {
                let mut card = card_arc.lock().await;
                card.attached.take()
            } {
                // Find the index of the attached card
                if let Some(attached_index) = self
                    .cards_in_play
                    .iter()
                    .position(|c| Arc::ptr_eq(c, &attached_arc))
                {
                    // Add the attached card index to the list to be detached
                    indices_to_detach.push(attached_index);
                }
            }

            // Optionally, collect actions triggered by the card's detachment
            // actions.extend(self.collect_detach_actions(card_arc.clone()).await);
        }

        actions
    }

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

        let mut stat_map: HashMap<StatType, i8> = HashMap::new();
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

    pub async fn choose_action(&self, game: &Game) -> Option<Arc<dyn Action + Send + Sync>> {
        // Collect available actions
        let available_actions = self.collect_available_actions(game).await;

        // For testing, let's assume the player always passes if they have no actions
        if available_actions.is_empty() {
            println!(
                "{} has no available actions and passes priority.",
                self.name
            );
            None // Player passes
        } else {
            // For the active player, you might want to simulate them taking an action or passing
            // For now, we'll simulate passing to avoid hanging
            println!("{} chooses to pass priority.", self.name);
            None // Player passes
        }
    }

    pub async fn execute_action(
        &mut self,
        index: usize,
        target: Option<EffectTarget>,
        game: &mut Game,
    ) -> Result<Vec<Arc<dyn Action + Send + Sync>>, String> {
        let mut actions = {
            let card = &self.cards_in_play[index];
            let card = card.clone();
            let mut card_l = card.lock().await;
            card_l.action_target = target;
            card_l
                .collect_manual_actions(
                    Arc::clone(&card),
                    game.current_turn.as_ref().unwrap().phase,
                )
                .await
        };
        println!("{:?}", actions);

        Ok(actions)
    }

    pub async fn tap_card(
        &mut self,
        index: usize,
        target: Option<EffectTarget>,
        game: &mut Game,
    ) -> Result<Vec<Arc<dyn Action + Send + Sync>>, String> {
        let card = &self.cards_in_play[index];
        let mutex = card.clone();
        let mut card_l = mutex.lock().await;

        card_l.action_target = target;
        card_l.tap()?;
        // let actions = Card::collect_phase_based_actions(
        //     &card,
        //     game.current_turn.as_ref().unwrap(),
        //     ActionTriggerType::Manual,
        // )
        // .await;

        let mut actions = card_l
            .collect_manual_actions(Arc::clone(card), game.current_turn.as_ref().unwrap().phase)
            .await;

        Ok(actions)
    }

    pub async fn play_card(
        player_arc: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
        game: &Game,
    ) -> Result<Vec<Arc<dyn Action + Send + Sync>>, String> {
        // Lock the player to mutate state
        let (card, can_cast, name) = {
            let mut player = player_arc.lock().await;
            let card = player.cards_in_hand.remove(index);
            let can_cast = player.can_cast(&card).await;
            let name = player.name.clone();
            (card, can_cast, name)
        }; // Lock is released here

        if !can_cast {
            // Need to lock again to insert the card back
            let mut player = player_arc.lock().await;
            player.cards_in_hand.insert(index, Arc::clone(&card)); // Put the card back in hand
            return Err(format!(
                "Not enough mana to cast card {} for {}",
                card.lock().await.name,
                name
            ));
        }

        // Pay mana
        {
            let mut player = player_arc.lock().await;
            player.pay_mana_for_card(&card).await;
        } // Lock is released here

        // Set target and owner
        {
            let mut card_lock = card.lock().await;
            card_lock.target = target;
            card_lock.owner = Some(player_arc.clone());
        } // Lock is released here

        // Add card to cards_in_play
        {
            let mut player = player_arc.lock().await;
            player.cards_in_play.push(Arc::clone(&card));
        } // Lock is released here

        // Collect actions without holding any locks
        let mut actions = {
            let card_lock = card.lock().await;
            card_lock.collect_phase_based_actions_sync(
                &game.current_turn.clone().unwrap(),
                ActionTriggerType::Instant,
                &card,
            )
        };

        // Additional logic
        {
            let card_lock = card.lock().await;
            if card_lock.card_type == CardType::Instant {
                actions.push(Arc::new(CardActionWrapper {
                    action: Arc::new(DestroySelfAction {}),
                    card: Arc::clone(&card),
                }));
            }
        } // Lock is released here

        Ok(actions)
    }

    pub fn draw_card(&mut self) {
        println!("{} draws a card.", self.name);
        if let Some(card) = self.deck.draw() {
            self.cards_in_hand.push(card);
        }
    }

    pub async fn destroy_card_in_play(
        &mut self,
        card_index: usize,
        turn: &Turn,
        // game: &mut Game,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let card = &self.cards_in_play.remove(card_index);
        self.deck.destroy(Arc::clone(card));

        // let card = card.lock().await;
        let actions =
            Card::collect_phase_based_actions(card, turn, ActionTriggerType::OnCardDestroyed).await;

        actions
    }

    pub async fn collect_available_actions(
        &self,
        game: &Game,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions = Vec::new();
        let turn_phase = game.current_turn.as_ref().unwrap().phase;

        // Collect manual actions from cards in play
        for card_arc in &self.cards_in_play {
            let card = card_arc.lock().await;
            let mut card_actions = card
                .collect_manual_actions(Arc::clone(card_arc), turn_phase)
                .await;
            actions.append(&mut card_actions);
        }

        // Add other player actions if needed

        actions
    }

    pub async fn collection_actions_for_phase(
        player: Arc<Mutex<Player>>,
        player_index: usize,
        turn: Turn,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions_for_phase = Vec::new();
        let pla = player.lock().await;

        for trigger in &pla.triggers {
            let action = trigger;

            if trigger
                .applies_in_phase(turn.clone(), Arc::clone(&player))
                .await
            {
                actions_for_phase.push(Arc::new(PlayerActionWrapper {
                    action: Arc::clone(&action.action), // Clone the Arc, not the action itself
                    player_index,
                })
                    as Arc<(dyn Action + std::marker::Send + Sync)>);
            }
        }

        actions_for_phase
    }

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

    pub(crate) fn from_claims(user: &crate::services::jwt::Claims) -> Player {
        Player::new(&user.sub, 100, 10, vec![])
    }

    pub async fn empty_mana_pool(&mut self) {
        self.mana_pool.empty_pool();
    }

    pub fn display_mana_pool(&self) -> String {
        self.mana_pool.format_mana()
    }

    pub async fn has_required_mana(&self, requirement: &Vec<ManaType>) -> bool {
        let mut required_mana = ManaPool::new();

        for mana in requirement {
            required_mana.add_mana(*mana);
        }

        self.mana_pool.white >= required_mana.white
            && self.mana_pool.blue >= required_mana.blue
            && self.mana_pool.black >= required_mana.black
            && self.mana_pool.red >= required_mana.red
            && self.mana_pool.green >= required_mana.green
            && self.mana_pool.colorless >= required_mana.colorless
    }

    pub async fn can_cast(&self, card: &Arc<Mutex<Card>>) -> bool {
        let card = card.lock().await;

        self.has_required_mana(&card.cost).await
    }

    pub async fn pay_mana_for_card(&mut self, card: &Arc<Mutex<Card>>) {
        let card = card.lock().await;

        // Deduct the mana from the player's pool according to the card's cost
        for mana in &card.cost {
            match mana {
                ManaType::White => self.mana_pool.white -= 1,
                ManaType::Blue => self.mana_pool.blue -= 1,
                ManaType::Black => self.mana_pool.black -= 1,
                ManaType::Red => self.mana_pool.red -= 1,
                ManaType::Green => self.mana_pool.green -= 1,
                ManaType::Colorless => self.mana_pool.colorless -= 1,
            }
        }

        println!(
            "Player {} paid {} mana for {}",
            self.name,
            card.format_mana_cost(), // Assuming card has a render_mana_cost method for formatted mana display
            card.name
        );
    }
}

impl Stats for Player {
    fn add_stat(&mut self, id: String, stat: Stat) {
        self.stat_manager.add_stat(id, stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        self.stat_manager.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
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
