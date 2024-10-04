use std::{
    borrow::{Borrow, BorrowMut}, cell::RefCell, collections::HashMap, fmt, rc::Rc, sync::Arc, thread::Thread, time::Duration
};

use action::{Action, CardActionTarget, CardActionWrapper, CardRequiredTarget, CombatDamageAction, DestroySelfAction, TriggerTarget};
use card::{Card, CardPhase, CardType};
use combat::Combat;
use effects::{EffectID, EffectManager, EffectTarget};
use mana::ManaPool;
use player::Player;
use serde::{Deserialize, Serialize};
use specta::Type;
use stat::{StatManager, StatType, Stats};
use tokio::{select, sync::{broadcast, mpsc, Mutex, Notify, RwLock}, time::{sleep, timeout, Instant}};
use turn::{Turn, TurnPhase};

pub mod combat;
pub mod mana;
pub mod action;
pub mod card;
pub mod deck;
pub mod effects;
pub mod player;
pub mod stat;
pub mod turn;



enum PhaseAction {
    Untap(usize),
    Upkeep(usize),
    Draw(usize),
    Main(usize),
    Combat(usize),
    End(usize),
    CardPhaseChange(usize, TurnPhase),
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum GameStatus {
    NeedsPlayers,
    InGame,
    WaitingForStart(i32),
}

impl Default for GameStatus {
    fn default() -> Self {
        GameStatus::NeedsPlayers
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, Default)]
pub struct GameState {
    pub players: HashMap<String, PlayerState>,
    pub public_info: PublicGameInfo,
    pub status: GameStatus,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum PlayerStatus {
    Spectator,
    Ready,
    InGame,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum ActionType {
    Tap,
    None,
    Instant,
    Attach,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct CardWithDetails {
    pub card: Card,
    pub required_target: CardRequiredTarget,
    pub action_type: ActionType,
    // pub actionable_target: CardActionTarget,
}

impl CardWithDetails {
    fn get_required_target(card: &Card,


        turn_phase: TurnPhase,

    ) -> (CardRequiredTarget, ActionType) {

        if card.current_phase != CardPhase::Ready {
            return (CardRequiredTarget::None, ActionType::None);
        }

        for trigger in card.triggers.iter() {
            match &trigger.trigger_type {
                action::ActionTriggerType::Tap => {
                    return (trigger.card_required_target.clone(), ActionType::Tap)
                },
                action::ActionTriggerType::TapWithinPhases(allowed_phases) => {
                    if allowed_phases.contains(&turn_phase) {

                        return (trigger.card_required_target.clone(), ActionType::Tap);
                    }
                }
,
                action::ActionTriggerType::Instant => {
                    if &trigger.card_required_target != &CardRequiredTarget::None {
                        return (trigger.card_required_target.clone(), ActionType::Instant);
                    }
                }

                action::ActionTriggerType::Attached => {
                    if &turn_phase == &TurnPhase::Main {
                        return (trigger.card_required_target.clone(), ActionType::Attach);
                    }

                }

                action::ActionTriggerType::ManualWithinPhases(required_mana, allowed_phases) => {
                    if allowed_phases.contains(&turn_phase) {
                        return (trigger.card_required_target.clone(), ActionType::Instant);
                    }

                }
                x => {
                    println!("make actionable? {:?}", x);
                    // actionable = false;
                }
            }
        }

        (CardRequiredTarget::None, ActionType::None)
    }

    pub fn from_card(card: Card, turn_phase: TurnPhase) -> CardWithDetails {
        let (required_target, action_type) = CardWithDetails::get_required_target(&card, turn_phase);
        CardWithDetails { card, required_target, action_type }
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PlayerState {
    pub public_info: PublicPlayerInfo,
    pub hand: Vec<CardWithDetails>,
    pub discard_pile: Vec<CardWithDetails>,
    pub status: PlayerStatus,
    pub is_leader: bool,
    pub player_index: i32,

    #[serde(skip_serializing, skip_deserializing)]
    pub player: Arc<Mutex<Player>>,
}
impl PlayerState {
    pub(crate) fn from_player(player: Arc<Mutex<Player>>, player_index: i32) -> PlayerState {
        PlayerState {
            public_info: PublicPlayerInfo {
                cards_in_play: vec![],
                spells: vec![],
                hand_size: 0 ,
                mana_pool: ManaPool::new(),
                health: 10,
            },
            hand: vec![],
            discard_pile: vec![],
            status: PlayerStatus::Spectator,
            player,
            is_leader: false,player_index,
        }
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, Default)]
pub struct PublicGameInfo {
    pub current_turn: Option<Turn>,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PublicPlayerInfo {
    pub hand_size: i32,
    pub cards_in_play: Vec<CardWithDetails>,
    pub spells: Vec<CardWithDetails>,
    pub mana_pool: ManaPool,
    pub health: i8
}



#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Game {
    #[serde(skip_serializing, skip_deserializing)]
    pub players: Vec<Arc<Mutex<Player>>>,
    pub current_turn: Option<Turn>,
    pub turn_number: usize,
    #[serde(skip_serializing, skip_deserializing)]
    pub action_queue: Vec<Arc<dyn Action + Send +Sync>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub effect_manager: EffectManager,
    #[serde(skip_serializing, skip_deserializing)]
    pub event_stack: Vec<Arc<dyn Action + Send + Sync>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub combat: Combat,
    #[serde(skip_serializing, skip_deserializing)]
    current_priority_player: Option<(Arc<Mutex<Player>>, bool)>,
    #[serde(skip_serializing, skip_deserializing)]
    pub turn_change_sender: Option<broadcast::Sender<()>>,
}

impl Game {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100); // buffer size of 100

        Self {
            players: vec![],
            current_turn: None,
            turn_number: 0,
            action_queue: vec![],
            effect_manager: EffectManager::new(),
            event_stack: vec![],
            combat: Combat::new(),
            current_priority_player: None,
            turn_change_sender: Some(sender),
        }
    }

    pub fn add_to_stack(&mut self, action: Arc<dyn Action + Send + Sync>) {
        self.event_stack.push(action);
    }

    pub async fn reset_creature_damage(&mut self) {
        for player_arc in &self.players {
            let mut player = player_arc.lock().await;
            for card_arc in &player.cards_in_play {
                let mut card = card_arc.lock().await;
                if card.card_type == CardType::Creature {
                    card.damage_taken = 0;
                }
            }
        }
    }

    pub async fn resolve_stack(&mut self) {
        while let Some(action) = self.event_stack.pop() {
            action.apply(self).await;
        }

        for player_arc in &self.players {
            let mut player = player_arc.lock().await;
            player.reset_spells();
        }
    }

    pub async fn detach_cards_from(&mut self, card: &Arc<Mutex<Card>>) {
        let mut actions = vec![];
        let mut detach_list = vec![]; // List of (player_arc, card_indices_to_detach)

        // Step 1: Collect cards to detach
        for player_arc in &self.players {
            let (player_name, card_indices_to_detach) = {
                let player = player_arc.lock().await;
                let mut indices = Vec::new();

                for (index, card_in_play_arc) in player.cards_in_play.iter().enumerate() {
                    let should_detach = {
                        let card_in_play = card_in_play_arc.lock().await;

                        let is_attached = if let Some(attached_arc) = &card_in_play.attached {
                            Arc::ptr_eq(attached_arc, card)
                        } else {
                            false
                        };

                        let is_same_card = Arc::ptr_eq(card_in_play_arc, card);

                        is_attached || is_same_card
                    };

                    if should_detach {
                        indices.push(index);
                    }
                }

                (player.name.clone(), indices)
            }; // Locks are released here

            if !card_indices_to_detach.is_empty() {
                detach_list.push((player_arc.clone(), card_indices_to_detach));
            }
        }

        // Step 2: Detach cards
        for (player_arc, mut indices) in detach_list {
            // Sort indices in reverse order to avoid shifting
            indices.sort_unstable_by(|a, b| b.cmp(a));

            let mut player = player_arc.lock().await;

            for index in indices {
                println!("Detaching card at index {} from player {}", index, player.name);

                let mut detach_actions = player.detach_card_in_play(index, self).await;
                actions.append(&mut detach_actions);
            }
        }

        // Execute all collected actions
        self.execute_actions(&mut actions).await;
    }

    pub async fn destroy_card(&mut self, card: &Arc<Mutex<Card>>) {
        self.detach_cards_from(card).await;
        let mut actions = vec![];
        for player in self.players.iter() {
            let cards_to_destroy = {
                let player_locked = player.lock().await;

                let mut cards_to_destroy = Vec::new();
                for (card_index, card_in_play) in player_locked.cards_in_play.iter().enumerate() {
                    // println!("ah {:?}", card_in_play.lock().await.attached_to);
                    if Arc::ptr_eq(card, card_in_play) {
                        cards_to_destroy.push(card_index);
                    }
                }

                cards_to_destroy // Return the indices of cards to destroy
            };

            for &card_index in &cards_to_destroy {
                let mut player_locked = player.lock().await;
                actions.append(&mut player_locked.destroy_card_in_play(card_index, self.current_turn.as_ref().unwrap()).await);
            }

        }

        self.execute_actions(&mut actions).await;
    }


    pub async fn add_player(&mut self, player: Player) -> Arc<Mutex<Player>> {
        let player_arc = Arc::new(Mutex::new(player));
        player_arc.lock().await.deck.set_owner(&player_arc).await;
        self.players.push(Arc::clone(&player_arc));

        player_arc
    }

    pub async fn attach_card_action(
        &mut self,
        player: &Arc<Mutex<Player>>,
        in_play_index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {

        target.clone().ok_or_else(|| "Choose a target".to_string())?;

        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked.attach_card(in_play_index, target, self).await?
        };
        println!("actions {:?}", actions);

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn activate_card_action(
        &mut self,
        player: &Arc<Mutex<Player>>,
        in_play_index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        println!("player {:?}", player);
        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked.execute_action(in_play_index, target, self).await?
        };

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn tap_card(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked.tap_card(index, target, self).await?
        };

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn play_card(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        if let Some((current_player, action_taken)) = &mut self.current_priority_player {
            if !Arc::ptr_eq(&player, current_player) {
                return Err("Not your turn".to_string());
            } else {
                *action_taken = true; // Mark that an action has been taken
            }
        }


        // Attempt to play the card
        let action = {
            Player::play_card(player, index, target, self.current_turn.clone().unwrap()).await?
        };


        // Add the action to the stack
        self.add_to_stack(action);
        println!("Action added to stack");

        // Do not spawn the priority loop here; let the caller handle it

        Ok(())
    }

    pub async fn next_priority_queue(&mut self) {
        // if no one in queue, set to first person
        // else, set to next person

        // in the background, set something that will mutate self to move the queue after X?
        // or have the lobby have something that will call this after X?
    }


    pub async fn collect_actions_for_phase(&mut self) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions = Vec::new();

        for (player_index,player) in self.players.iter().enumerate() {
            let mut a = Player::collection_actions_for_phase(Arc::clone(player), player_index, self.current_turn.clone().unwrap()).await;
            actions.append(&mut a);


            for card_rc in &player.lock().await.cards_in_play {
                // let card = Arc::clone(card_rc);
                // let card = card.lock().await;
                let collected_actions: Vec<Arc<dyn Action + Send + Sync>> = Card::collect_phase_based_actions(
                    card_rc,
                    &self.current_turn.clone().unwrap(),
                    action::ActionTriggerType::PhaseBased(vec![], TriggerTarget::Any),
                ).await;
                actions.extend(collected_actions);
                // let effects: Vec<EffectID> = self.effect_manager.effects.keys().clone().into_iter().map(|x| x.clone()).collect();
                let has_effects = self.effect_manager.has_effects(card_rc).await;
                if card_rc.lock().await.is_useless(has_effects) {
                    println!("card is considered useless, let's get rid of it");
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(DestroySelfAction {}),
                        card: Arc::clone(card_rc)
                    }));
                }
            }
        }

        actions
    }


    pub async fn execute_actions(&mut self, actions: &mut Vec<Arc<dyn Action + Send +Sync>>) {
        let actions_to_execute = std::mem::take(actions);

        for action in actions_to_execute {
            println!("Applying action {:?}", action);
            action.apply(self).await;
        }

        self.effect_manager.apply_effects(self.current_turn.clone().unwrap()).await;
    }


    pub async fn start_turn(&mut self, player_index: usize) {
        // Set up the new turn
        self.current_turn = Some(Turn::new(
            self.players[player_index].clone(),
            player_index,
            self.turn_number,
        ));
        self.turn_number += 1;

        self.reset_creature_damage().await;

        // Advance card phases for the current player
        let player_arc = self.players[player_index].clone();
        {
            let mut player = player_arc.lock().await;
            player.advance_card_phases().await;
        }

        // Optionally, print player's state
        let player = self.players[player_index].lock().await;
        println!(
            "{}'s turn: ------\n{}",
            player.name,
            player.render(30, 10, 30).await
        );
    }

    // pub async fn apply_effects_for_phase(&mut self) {
    //     // Apply game-wide effects first (if any)
    //     if let Some(ref mut turn) = self.current_turn {
    //         let phase = turn.phase;
    //         let wat = self.effect_manager.get_effects_for_phase(phase);
    //         for effect in wat {
    //             effect.apply(&EffectTarget::Game).await;
    //         }

    //         // Collect player references first
    //         let player_refs: Vec<Arc<Mutex<Player>>> = self.players.iter().cloned().collect();

    //         // Apply player and card-specific effects
    //         for player in player_refs {
    //             // Apply effects to the player
    //             self.apply_player_effects_for_phase(&player, phase).await;

    //             // Apply effects to each card in play
    //             let player_locked = player.lock().await;
    //             for card in &player_locked.cards_in_play {
    //                 let card_arc = Arc::clone(card);
    //                 self.apply_card_effects_for_phase(&card_arc, phase).await;
    //             }
    //         }
    //     }
    // }

    pub async fn handle_deaths(&mut self) {
        // Collect player statuses asynchronously
        let mut alive_players = Vec::new();

        for player_arc in &self.players {
            let mut player = player_arc.lock().await;
            let health = player.get_stat_value(StatType::Health);
            if health <= 0 {
                player.is_alive = false;
                println!("{} has been defeated.", player.name);
            } else {
                alive_players.push(player_arc.clone());
            }
        }

        // Update the players list
        self.players = alive_players;

        // Optionally, handle defeated players (e.g., remove their cards)
    }


    pub async fn execute_player_action(
        &mut self,
        player_arc: Arc<Mutex<Player>>,
        action: Arc<dyn Action + Send + Sync>,
    ) -> Result<(), String> {
        // Apply the action
        action.apply(self).await;

        Ok(())
    }

    pub fn get_players_in_priority_order(&self) -> Vec<Arc<Mutex<Player>>> {
        // Assuming the active player is first
        let mut players_in_order = Vec::new();

        if let Some(ref turn) = self.current_turn {
            // Start with the active player
            // players_in_order.push(turn.current_player.clone());

            // Add other players in order
            for player_arc in &self.players {
                if !Arc::ptr_eq(player_arc, &turn.current_player) {
                    players_in_order.push(player_arc.clone());
                }
            }
        }

        players_in_order
    }

    pub async fn wait_for_player_action_async(
        game_arc: Arc<RwLock<Game>>,
        time_limit: u64,
    ) -> bool {
        let action_future = async {
            let mut elapsed_time = 0u64;
            let mut time_since_last_notification = 0u64;
            let sleep_duration = 100u64; // milliseconds

            loop {
                // Check if an action has been performed
                {
                    let game = game_arc.read().await;
                    if game.performed_action() {
                        return true;
                    }
                } // Read lock released here

                // Sleep for a short duration to avoid busy-waiting
                sleep(Duration::from_millis(sleep_duration)).await;
                elapsed_time += sleep_duration;
                time_since_last_notification += sleep_duration;

                if time_since_last_notification >= 1000 {
                    time_since_last_notification = 0;
                    // Send a notification every second
                    let sender = {
                        let game = game_arc.read().await;
                        game.turn_change_sender.clone()
                    }; // Read lock released here

                    if let Some(sender) = sender {
                        let _ = sender.send(()); // Ignore errors if no receivers
                    }
                }

                // Check if we've exceeded the time limit
                if elapsed_time >= time_limit * 1000 {
                    return false;
                }
            }
        };

        match timeout(Duration::from_secs(time_limit), action_future).await {
            Ok(action_performed) => action_performed,
            Err(_) => {
                // Timeout occurred
                false
            }
        }
    }


    pub  fn performed_action(&self) -> bool {
        if let Some((_, action)) = &self.current_priority_player {
            return *action;
        }
        false
    }

    pub async fn priority_loop(game_arc: Arc<RwLock<Game>>) {
        println!("Entering priority loop");

        // Get players in order without holding the lock
        let players_in_order = {
            let game = game_arc.read().await;
            game.get_players_in_priority_order()
        };

        let num_players = players_in_order.len();
        let mut passed_players = vec![false; num_players];

        loop {
            let mut all_passed = true;

            for (i, player_arc) in players_in_order.iter().enumerate() {
                if passed_players[i] {
                    continue;
                }

                // Start the player's priority turn
                {
                    let mut player = player_arc.lock().await;
                    player.priority_turn_start().await;
                    println!("Player {}'s priority turn has started.", player.name);
                }

                // Set the current priority player
                {
                    let mut game = game_arc.write().await;
                    game.current_priority_player = Some((player_arc.clone(), false));

                    // Notify that the player's priority turn has started
                    if let Some(ref sender) = game.turn_change_sender {
                        let _ = sender.send(()); // Ignore errors if no receivers
                    }
                } // Write lock released here

                // Wait for player action or timeout
                let time_limit = if i == 0 { 10 } else { 3 };
                let game_arc_clone = Arc::clone(&game_arc);
                let action_performed = Game::wait_for_player_action_async(game_arc_clone, time_limit).await;

                {
                    let mut player = player_arc.lock().await;
                    if action_performed {
                        println!("{} performed an action.", player.name);
                        all_passed = false; // Player performed an action
                    } else {
                        // Player took no action within the time limit
                        println!("{} did not act in time, passing.", player.name);
                        passed_players[i] = true;
                    }

                    // End the player's priority turn
                    player.priority_turn_end().await;
                }
            }

            if passed_players.iter().all(|&passed| passed) {
                println!("All players have passed. Exiting priority loop.");

                // Notify that the priority loop has ended
                {
                    let game = game_arc.read().await;
                    if let Some(ref sender) = game.turn_change_sender {
                        let _ = sender.send(()); // Notify listeners
                    }
                }

                break;
            }
        }

        // Clear the current priority player
        {
            let mut game = game_arc.write().await;
            game.current_priority_player = None;
        }
    }

    pub async fn process_action_queue(game_arc: Arc<RwLock<Game>>) {
        // Run the priority loop
        Self::priority_loop(Arc::clone(&game_arc)).await;

        let mut game = game_arc.write().await;
        // Resolve the stack
        game.resolve_stack().await;

        // After resolving the stack, you might want to notify again
        if let Some(ref sender) = game.turn_change_sender {
            let _ = sender.send(()); // Notify that the stack has been resolved
        }
    }


    pub async fn advance_turn(&mut self) {
        if let Some(ref mut turn) = self.current_turn {
            if let Some(ref mut turn) = self.current_turn {
                turn.next_phase();
                if turn.phase == TurnPhase::Untap {
                    let next_player_index = (turn.current_player_index + 1) % self.players.len() as i32;
                    self.start_turn(next_player_index as usize).await;
                }
            }

            let mut actions = self.collect_actions_for_phase().await;
            self.execute_actions(&mut actions).await;
        }

        println!(":: TURN ADVANCED :: {:?} effects: {:?}", self.current_turn.clone().unwrap().phase, self.effect_manager.effects);
    }

    fn current_phase(&self) -> TurnPhase {
        self.current_turn.as_ref().unwrap().phase
    }

    pub(crate) async fn start(&mut self) {
        for player in &self.players {
            let mut player = player.lock().await;
            for _ in 0..6 {
                player.draw_card();
            }
        }
        self.start_turn(0).await;
    }
}
mod test {
    use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

    use tokio::sync::{Mutex, Notify};

    use crate::game::{action::{ generate_mana::GenerateManaAction, ActionTriggerType, ApplyStat, CardActionTrigger, CardActionWrapper, CardRequiredTarget, CounterSpellAction, DeclareAttackerAction, DeclareBlockerAction, DestroyTargetCAction, PlayerActionTarget, PlayerActionTrigger}, card::{self, card::create_creature_card}, deck::Deck, effects::{ApplyEffectToCardBasedOnTotalCardType, ApplyEffectToPlayerCardType, ApplyEffectToTargetAction, EffectTarget, ExpireContract, StatModifierEffect}, mana::ManaType};

    use super::{
        action::{add_stat::CardAddStatAction, CardActionTarget, CardDamageAction, TriggerTarget}, card::{Card, CardPhase, CardType}, effects::{
        }, player::Player, stat::{Stat, StatType}, turn::{Turn, TurnPhase}, Game
    };

    #[tokio::test]
    async fn test_sorcery() {
        let mut game = Game::new();
        game.add_player(Player::new("tim", 20, 0, vec![
            card::card::create_creature_card!(
                "A wall",
                "A brick wall",
                1,
                7,
                []
            ),
            Card::new(
                "Overrun",
                "Creatures you control get +3/+3 and trample",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![],
            ),
        ])).await;

        game.start_turn(0).await;

        let player = Arc::clone(&game.players[0]);
        player.lock().await.draw_card();
        player.lock().await.draw_card();
        game.play_card(&player, 1, None).await.unwrap();

        // let target_card = {
        //     let card = &player.lock().await.cards_in_play[0];
        //     Arc::clone(card)
        // };
        // let target = Some(EffectTarget::Card(target_card));
        game.play_card(&player, 0, None).await.unwrap();
        println!("{}", player.lock().await.render(30, 10, 30).await);

        for _ in 0..15 {
        game.advance_turn().await;
        }

    }


    #[tokio::test]
    async fn test_counter() {
        let mut game = Game::new();

        // Adding two players: tim and tim troll
        game.add_player(Player::new("tim", 20, 0, vec![
            card::card::create_creature_card!(
                "A wall",
                "A brick wall",
                1,
                7,
                []
            ),
        ])).await;

        game.add_player(Player::new("tim troll", 20, 0, vec![
            Card::new(
                "Counter Spell",
                "Counter target spell.",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Instant,
                    CardRequiredTarget::Spell,
                    Arc::new(CounterSpellAction {}),
                )],
                CardPhase::Ready,
                CardType::Instant,
                vec![],
                vec![],
            ),
        ])).await;

        // Start the turn
        game.start_turn(0).await;

        // Simulate drawing a card for both players
        let player = Arc::clone(&game.players[0]);
        let troll = Arc::clone(&game.players[1]);
        player.lock().await.draw_card();
        troll.lock().await.draw_card();

            let target_card = {
                let card = &player.lock().await.cards_in_hand[0];
                Arc::clone(card)
            };
        // Simulate 'tim' playing a creature card (A wall)
        game.play_card( &player, 0, None).await.unwrap();

        // // Spawn the priority loop with the Notify object
        // let game_clone = Arc::clone(&game);
        // let notify_clone = Arc::clone(&notify);
        // let priority_loop_handle = tokio::spawn(async move {
        //     Game::priority_loop(game_clone, notify_clone).await;
        // });

        // Wait for "tim troll"'s priority turn to start
        let troll_clone = Arc::clone(&troll);
        // let handle = tokio::spawn(async move {
        //     loop {
        //         // Wait for the notification
        //         notify_clone.notified().await;

        //         // Check if it's "tim troll"'s turn
        //         let current_player = {
        //             let game = game_clone.lock().await;
        //             if let Some((current_player, _)) = &game.current_priority_player {
        //                 Some(Arc::clone(current_player))
        //             } else {
        //                 None
        //             }
        //         };

        //         if let Some(current_player) = current_player {
        //             let player_name = current_player.lock().await.name.clone();
        //             if player_name == "tim troll" {
        //                 // It's "tim troll"'s priority turn, play the counterspell

        //                 Game::play_card_queue(Arc::clone(&game_clone), &troll_clone, 0, Some(EffectTarget::Card(target_card)), notify).await.unwrap();
        //                 break;
        //             }
        //         }
        //     }
        // });

        // Wait for the background tasks to complete
        // handle.await.unwrap();
        // priority_loop_handle.await.unwrap();

        // Render game state after actions
        println!("{}", player.lock().await.render(30, 10, 30).await);
    }

    #[tokio::test]
    async fn test_enchantments() {
        let mut game = Game::new();
        game.add_player(Player::new("tim", 20, 0, vec![
            card::card::create_creature_card!(
                "A wall",
                "A brick wall",
                1,
                7,
                [ManaType::Green]
            ),
            Card::new(
                "Rancor",
                "Enchanted creature gets +2/+0 and trample",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    2,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Enchantment,
                vec![],
                vec![],
            ),
            Card::new(
                "Blanchwood Armor",
                "Enchanted creature gets +1/+1 for each Forest you control",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Attached,
                    CardRequiredTarget::CardOfType(CardType::Creature),
                    Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                        card_type: CardType::BasicLand(ManaType::Green),
                        effect_generator: Arc::new(|target, source_card, total| {
                            let effect = StatModifierEffect::new(
                                target,
                                StatType::Defense,
                                total,
                                ExpireContract::Never,
                                source_card,
                            );
                            Arc::new(Mutex::new(effect))
                        }),
                    }),
                )],
                CardPhase::Ready,
                CardType::Enchantment,
                vec![],
                vec![ManaType::Green],
            ),
        ])).await;

        game.start_turn(0).await;

        let creature_card = game.players[0].lock().await.deck.draw_pile.remove(0);
        let enchantment = game.players[0].lock().await.deck.draw_pile.remove(0);
        let player = Arc::clone(&game.players[0]);
        player.lock().await.cards_in_play.push(Arc::clone(&creature_card));
        player.lock().await.cards_in_hand.push(enchantment);

        game.play_card( &player, 0, None).await.unwrap();
        let target = Some(EffectTarget::Card(creature_card));
        game.attach_card_action(&player, 1, target).await.unwrap();
        println!("{}", player.lock().await.render(30, 10, 30).await);

    }

    #[tokio::test]
    async fn test() {


        let mut game = Game::new();

        game.add_player(
            Player::new(
                "Player 1",
                100,
                5,
                vec![
                    card::card::create_creature_card!(
                        "A wall",
                        "A brick wall",
                        1,
                        7,
                        [ManaType::Black]
                    ),
                    Card::new(
                        "Buff",
                        "Adds 1 damage stat to a card",
                        vec![
                            CardActionTrigger::new(
                                ActionTriggerType::Instant,
                                CardRequiredTarget::CardOfType(CardType::Creature),
                                Arc::new(ApplyEffectToTargetAction {
                                    effect_generator: Arc::new(|target, source_card| {
                                        let effect = StatModifierEffect::new(target, StatType::Damage, 1, ExpireContract::Never, source_card);
                                        Arc::new(Mutex::new(effect))
                                    }),
                                }),
                            ),
                        ],
                        CardPhase::Ready,
                        CardType::Enchantment,
                        vec![],
                        vec![ManaType::Black],
                    ),
                    Card::new(
                        "Poison",
                        "Applies 2 damage for the next 3 turns after 1 turn.",
                        vec![
                            CardActionTrigger::new(ActionTriggerType::PhaseBased(vec![TurnPhase::Upkeep], TriggerTarget::Owner),
                                CardRequiredTarget::EnemyPlayer,
                                Arc::new(CardDamageAction {
                                    target: CardActionTarget::CardTarget
                                })
                            )
                        ],
                        CardPhase::Charging(1),
                        CardType::Artifact,
                        vec![Stat::new(StatType::Damage, 2)],
                        vec![ManaType::Black]
                    ),

                    Card::new(
                        "Swamp",
                        "TAP: Adds 1 Swamp mana to your pool",
                        vec![
                            CardActionTrigger::new(ActionTriggerType::Tap,
                                CardRequiredTarget::None,
                             Arc::new(GenerateManaAction {
                                mana_to_add: vec![ManaType::Black],
                                target: PlayerActionTarget::SelfPlayer,
                            })),
                        ],
                        CardPhase::Ready,
                        CardType::Land(vec![ManaType::Black]),
                        vec![],
                        vec![]
                    ),
                ],
            )
        ).await;
        game.add_player(Player::new("Player 2", 100, 5, vec![
            Card::new(
                "Overrun",
                "Creatures you control get +3/+3 and trample",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    3,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    3,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                            card_type: CardType::BasicLand(ManaType::Blue),
                            effect_generator: Arc::new(|target, source_card, total| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    total,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![ManaType::Blue],
            ),

            create_creature_card!(
                "Kalonian Tusker",
                "Big creature with raw power",
                6,
                1,
                [ManaType::White, ManaType::Blue]
            ),

            Card::new(
                "Plains",
                "TAP: Adds 1 white mana to your pool",
                vec![
                    CardActionTrigger::new(ActionTriggerType::Tap,
                        CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::White],
                        target: PlayerActionTarget::SelfPlayer,
                    })),
                ],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::White),
                vec![],
                vec![]
            ),

            Card::new(
                "Destroy",
                "Destroy target card",
                vec![
                            CardActionTrigger::new(ActionTriggerType::Instant, CardRequiredTarget::AnyCard,
                                Arc::new(DestroyTargetCAction {})
                            )
                ],
                CardPhase::Charging(1),
                CardType::Instant,
                vec![Stat::new(StatType::Damage, 1)],
                vec![ManaType::Blue]
            ),


            Card::new(
                "Island",
                "TAP: Adds 1 blue mana to your pool",
                vec![
                    CardActionTrigger::new(ActionTriggerType::Tap, CardRequiredTarget::None, Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Blue],
                        target: PlayerActionTarget::SelfPlayer,
                    })),
                ],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Blue),
                vec![],
                vec![]
            ),
        ])).await;



        game.start_turn(0).await;
        for _ in 0..23 {
            game.advance_turn().await;
        }


        let player = &game.players[0].clone();
        let player2 = &game.players[1].clone();
        let posion_target = EffectTarget::Player(Arc::clone(player2));
        game.play_card( player, 0, None).await.expect("oh no, no mana?");
        for _ in 0..25 {
            game.advance_turn().await;
        }

        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        // // Card::tap(&player.lock().await.cards_in_play[0], &mut game, player).await;
        println!("{}", player.lock().await.render(30, 10, 30).await);
        game.play_card( player, 0, Some(posion_target)).await.expect("oh no, no mana?");

        for _ in 0..15 {
            game.advance_turn().await;
        }
        // println!("{:?}", player.lock().await.cards_in_play[1].lock().await.triggers);
        let target_card = {
            let card = &player.lock().await.cards_in_play[1];
            Arc::clone(card)
        };

        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        game.play_card( player, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");
        for _ in 0..25 {
            game.advance_turn().await;
        }
        game.play_card( player2, 0, None).await.expect("oh no");
        game.tap_card(player2, 0, None).await.expect("unable to tap, no ready?");
        let target_card = {
            let card = &player.lock().await.cards_in_play[2];
            Arc::clone(card)
        };
        game.play_card( player2, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");

        for _ in 0..25 {
            game.advance_turn().await;
        }

        // play white mana
        game.play_card( player2, 0, None).await.expect("oh no");
        // tap blue mana
        game.tap_card(player2, 0, None).await.expect("unable to tap, no ready?");
        // tap white mana
        game.tap_card(player2, 1, None).await.expect("unable to tap, no ready?");
        // play creature
        game.play_card( player2, 0, None).await.expect("oh no");

        for _ in 0..22 {
            game.advance_turn().await;
        }
        println!("Declaring attackers....");
        game.activate_card_action(player2, 2, Some(EffectTarget::Player(Arc::clone(player)))).await.expect("hmmmmm?");
        for _ in 0..8 {
            game.advance_turn().await;
        }
        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        game.play_card( player, 0, None).await.expect("hmm");
        for _ in 0..10 {
            game.advance_turn().await;
        }
        game.tap_card(player2, 0, None).await.expect("unable to tap, no ready?");
        game.play_card( player2, 0, None).await.expect("nice");
        // println!("hello??");
        game.advance_turn().await;
        game.advance_turn().await;
        game.activate_card_action(player2, 2, Some(EffectTarget::Player(Arc::clone(player)))).await.expect("hmmmmm?");
        game.advance_turn().await;
        let target_card = {
            let card = &player2.lock().await.cards_in_play[2];
            Arc::clone(card)
        };
        game.activate_card_action(player, 2, Some(EffectTarget::Card(target_card))).await.expect("hmmmmm?");
        for _ in 0..24 {
            game.advance_turn().await;
        }

        // game.
        // {
        //     let player_arc = Arc::clone(player);
        //     tokio::spawn(async move {
        //         let mut player_locked = player_arc.lock().await;
        //         player_locked.choose_action(&game).await;
        //     }).await.unwrap();
        // }
        // player.choose_action(&game).await;

        // game.tap_card(player2, 2, Some(EffectTarget::Player(Arc::clone(player)))).await.expect("summoning sickness gone");
        // println!("Playing card{:?}\n\n\n\n\n\n\n\n", target_card);
        // game.play_card( player2, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");
    }

}
