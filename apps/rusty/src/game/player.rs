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
    game::{mana::ManaType, turn},
};

use super::{
    action::{
        Action, ActionTriggerType, CardActionWrapper, DrawCardAction, PlayerAction,
        PlayerActionTarget, PlayerActionTrigger, PlayerActionWrapper,
    },
    card::Card,
    deck::Deck,
    effects::{Effect, EffectTarget},
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
            deck: Deck::new(deck),
            triggers: vec![PlayerActionTrigger::new(
                super::action::ActionTriggerType::PhaseBased(
                    vec![TurnPhase::Draw],
                    super::action::TriggerTarget::Owner,
                ),
                Arc::new(DrawCardAction {
                    target: PlayerActionTarget::SelfPlayer,
                }),
            )],
            rendered_output: "".to_string(),
            mana_pool: ManaPool::new(),
        };

        player
    }

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
        for stat in &self.stat_manager.stats {
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

    pub async fn tap_card(
        player: &Arc<Mutex<Player>>,
        index: usize,
        game: &mut Game,
    ) -> Result<(), String> {
        let card = {
            let player = player.lock().await;
            Arc::clone(&player.cards_in_play[index])
        };

        if Card::tap(&card).await.is_ok() {
            let mut actions = Card::collect_phase_based_actions(
                &card,
                game.current_turn.as_ref().unwrap(),
                ActionTriggerType::Manual,
            )
            .await;

            game.execute_actions(&mut actions).await;
            // Card::trigger_tap(&card, game).await;
            Ok(())
        } else {
            Err("Card already tapped".to_string())
        }
    }

    pub async fn play_card(
        &mut self,
        index: usize,
        target: Option<EffectTarget>,
        game: &mut Game,
    ) -> Result<(Vec<Arc<dyn Action + Send + Sync>>), String> {
        let player = self;
        // let clone = Arc::clone(player);
        // let mut player = clone.lock().await;
        let card = &player.cards_in_hand.remove(index);

        // Step 1: Check if the player has enough mana to cast the card
        if !player.can_cast(card).await {
            // If not enough mana, return an error
            player.cards_in_hand.insert(index, Arc::clone(card)); // Put the card back in hand
            return Err(format!(
                "Not enough mana to cast card {} for {}",
                card.lock().await.name,
                player.name
            ));
        }

        // Step 2: Deduct the mana from the player's mana pool
        player.pay_mana_for_card(card).await;

        // Step 3: Set the target for the card
        println!(
            "{} targeted card \n {}\n at {:?}",
            player.name,
            card.lock().await.render(30).join("\n "),
            target
        );
        card.lock().await.target = target;

        // Step 4: Move the card from hand to play
        player.cards_in_play.push(Arc::clone(card));

        // Step 5: Collect and execute actions for the current phase
        let actions = Card::collect_phase_based_actions(
            card,
            &game.current_turn.clone().unwrap(),
            ActionTriggerType::Instant,
        )
        .await;

        println!("actions: {:?}", actions);

        Ok(actions)
    }

    // pub async fn collect_card_actions(
    //     &self,
    //     card_in_play_index: usize,
    //     game: &Game,
    // ) -> Vec<Arc<dyn Action + Send + Sync>> {
    //     let card = &self.cards_in_hand[card_in_play_index].clone();
    //     println!(
    //         "{} executed card \n {}",
    //         self.name,
    //         card.lock().await.render(30).join("\n ")
    //     );

    //     let mut actions_for_card = Vec::new();
    //     let owner = &game.current_turn.as_ref().unwrap().current_player;

    //     // Iterate over all player actions
    //     for action in card.lock().await.actions.iter() {
    //         // Check if the action applies to the current phase
    //         if action
    //             .lock()
    //             .await
    //             .applies_in_phase(
    //                 game.current_turn.as_ref().unwrap().clone(),
    //                 Arc::clone(owner),
    //             )
    //             .await
    //         {
    //             actions_for_card.push(Arc::new(CardActionWrapper {
    //                 card: Arc::clone(card),
    //                 action: Arc::clone(action),
    //                 owner: Arc::clone(owner),
    //             })
    //                 as Arc<(dyn Action + std::marker::Send + Sync + 'static)>);
    //         }
    //     }

    //     actions_for_card
    // }

    // pub fn untap(&mut self) {
    //     for card in self.cards_in_play.iter_mut() {
    //         card.untap();
    //     }
    // }

    pub fn draw_card(&mut self) {
        println!("{} draws a card.", self.name);
        if let Some(card) = self.deck.draw() {
            self.cards_in_hand.push(card);
        }
    }

    pub fn destroy_card_in_play(&mut self, card_index: usize) {
        let card = self.cards_in_play.remove(card_index);
        self.deck.destroy(card);
    }

    pub async fn collection_actions_for_phase(
        player: Arc<Mutex<Player>>,
        // &mut self,
        player_index: usize,
        turn: Turn,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions_for_phase = Vec::new();
        let pla = player.lock().await;

        // Iterate over all player actions
        for trigger in &pla.triggers {
            let action = trigger;

            // Arc::clone(card),
            // game.current_turn.clone().unwrap(),
            // game.current_turn.clone().unwrap().current_player.clone(),
            // Check if the action applies to the current phase
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

    pub async fn can_cast(&self, card: &Arc<Mutex<Card>>) -> bool {
        let card = card.lock().await;
        let mut required_mana = ManaPool::new();

        // Sum up the required mana from the card
        for mana in &card.cost {
            required_mana.add_mana(*mana);
        }

        // Check if the player's current mana pool has enough to pay the cost
        self.mana_pool.white >= required_mana.white
            && self.mana_pool.blue >= required_mana.blue
            && self.mana_pool.black >= required_mana.black
            && self.mana_pool.red >= required_mana.red
            && self.mana_pool.green >= required_mana.green
            && self.mana_pool.colorless >= required_mana.colorless
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
    fn add_stat(&mut self, stat: Stat) {
        self.stat_manager.add_stat(stat);
    }

    fn get_stat_value(&self, stat_type: StatType) -> i8 {
        self.stat_manager.get_stat_value(stat_type)
    }

    fn modify_stat(&mut self, stat_type: StatType, intensity: i8) {
        self.stat_manager.modify_stat(stat_type, intensity);
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
