use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::Arc,
};

use action::{Action, CombatDamageAction,  TriggerTarget};
use card::Card;
use combat::Combat;
use effects::{EffectManager, EffectTarget};
use player::Player;
use serde::{Deserialize, Serialize};
use specta::Type;
use stat::{StatType, Stats};
use tokio::sync::Mutex;
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

#[derive(Type, Deserialize, Serialize, Debug, Clone, Default)]
pub struct GameState {
    pub players: Vec<PlayerState>,
    pub public_info: PublicGameInfo,

}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PlayerState {
    pub user_id: String,
    pub hand: Vec<Card>,
    pub public_info: PublicPlayerInfo,

}

#[derive(Type, Deserialize, Serialize, Debug, Clone, Default)]
pub struct PublicGameInfo {

    pub current_turn: String,
    pub discard_pile: Vec<Card>,

}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PublicPlayerInfo {
    pub user_id: String,
    pub hand_size: usize,

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
}

impl Game {
    pub fn new() -> Self {
        Self {
            players: vec![],
            current_turn: None,
            turn_number: 0,
            action_queue: vec![],
            effect_manager: EffectManager::new(),
            event_stack: vec![],
            combat: Combat::new(),
        }
    }

    pub fn add_to_stack(&mut self, action: Arc<dyn Action + Send + Sync>) {
        self.event_stack.push(action);
    }

    pub async fn resolve_stack(&mut self) {
        while let Some(action) = self.event_stack.pop() {
            action.apply(self).await;
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


    pub async fn add_player(&mut self, player: Player) {
        let player_arc = Arc::new(Mutex::new(player));
        player_arc.lock().await.deck.set_owner(&player_arc).await;
        self.players.push(player_arc);
    }

    pub async fn activate_card_action(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked.execute_action(index, target, self).await?
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
        // Do not lock the player here
        let mut actions =
        Player::play_card(player, index, target, self)
            .await?;
        self.execute_actions(&mut actions).await;
        Ok(())
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
            }
        }

        actions
    }


    pub async fn execute_actions(&mut self, actions: &mut Vec<Arc<dyn Action + Send +Sync>>) {
        let actions_to_execute = std::mem::take(actions);

        for action in actions_to_execute {
            action.apply(self).await;
        }
    }


    pub async fn start_turn(&mut self, player_index: usize) {
        // Set up the new turn
        self.current_turn = Some(Turn::new(
            self.players[player_index].clone(),
            player_index,
            self.turn_number,
        ));
        self.turn_number += 1;

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
            players_in_order.push(turn.current_player.clone());

            // Add other players in order
            for player_arc in &self.players {
                if !Arc::ptr_eq(player_arc, &turn.current_player) {
                    players_in_order.push(player_arc.clone());
                }
            }
        }

        players_in_order
    }

    pub async fn priority_loop(&mut self) {
        println!("Entering priority loop");
        let players_in_order = self.get_players_in_priority_order();
        let num_players = players_in_order.len();
        let mut passed_players = vec![false; num_players];

        loop {
            let mut all_passed = true;

            for (i, player_arc) in players_in_order.iter().enumerate() {
                // Skip players who have already passed
                if passed_players[i] {
                    continue;
                }

                println!("Waiting for {} to choose an action.", player_arc.lock().await.name);

                // Let the player choose an action
                let action_option = {
                    let mut player = player_arc.lock().await;
                    player.choose_action(self).await
                }; // Lock on player is released here

                if let Some(action) = action_option {
                    // Player took an action
                    println!("{} took an action.", player_arc.lock().await.name);
                    self.add_to_stack(action);
                    passed_players = vec![false; num_players]; // Reset passed players
                    all_passed = false;
                } else {
                    // Player passed
                    println!("{} passed.", player_arc.lock().await.name);
                    passed_players[i] = true;
                }
            }

            if passed_players.iter().all(|&passed| passed) {
                println!("All players have passed. Exiting priority loop.");
                break;
            }
        }
    }

    pub async fn advance_turn(&mut self) {
        if let Some(ref mut turn) = self.current_turn {

            // Step 1: Collect actions for the current phase
            let mut actions = self.collect_actions_for_phase().await;

            // Step 2: Apply global effects for the current phase
            self.effect_manager.apply_effects().await;

            // Step 3: Execute collected actions for the current phase
            self.execute_actions(&mut actions).await;

            // Step 4: Move to the next phase
            if let Some(ref mut turn) = self.current_turn {
                turn.next_phase();

                // Step 5: If we're back to the Untap phase, move to the next player
                if turn.phase == TurnPhase::Untap {
                    let next_player_index = (turn.current_player_index + 1) % self.players.len();
                    self.start_turn(next_player_index).await;
                }
            }
        }

        // println!("{:?} effects: {:?}", self.current_turn.clone().unwrap().phase, self.effect_manager.effects);

    }

    fn current_phase(&self) -> TurnPhase {
        self.current_turn.as_ref().unwrap().phase
    }
}
mod test {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use tokio::sync::Mutex;

    use crate::game::{action::{ generate_mana::GenerateManaAction, ActionTriggerType, ApplyStat, CardActionTrigger, CardActionWrapper, DeclareAttackerAction, DeclareBlockerAction, DestroyTargetCAction, PlayerActionTarget, PlayerActionTrigger}, card, effects::{ApplyEffectToTargetAction, EffectTarget, StatModifierEffect}, mana::ManaType};

    use super::{
        action::{add_stat::CardAddStatAction, CardActionTarget, CardDamageAction, TriggerTarget}, card::{Card, CardPhase, CardType}, effects::{
        }, player::Player, stat::{Stat, StatType}, turn::{Turn, TurnPhase}, Game
    };

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
                        ManaType::Black
                    ),
                    Card::new(
                        "Buff",
                        "Adds 1 damage stat to a card",
                        vec![
                            CardActionTrigger::new(
                                ActionTriggerType::Instant,
                                Arc::new(ApplyEffectToTargetAction {
                                    effect_generator: Arc::new(|target, source_card| {
                                        let effect = StatModifierEffect::new(target, StatType::Damage, 1, None, source_card);
                                        Arc::new(Mutex::new(effect))
                                    }),
                                }),
                            ),
                        ],
                        CardPhase::Ready,
                        CardType::Equipment,
                        vec![],
                        vec![ManaType::Black],
                    ),
                    Card::new(
                        "Poison",
                        "Applies 2 damage for the next 3 turns after 1 turn.",
                        vec![
                            CardActionTrigger::new(ActionTriggerType::PhaseBased(vec![TurnPhase::Upkeep], TriggerTarget::Owner),
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
                            CardActionTrigger::new(ActionTriggerType::Tap, Arc::new(GenerateManaAction {
                                mana_to_add: vec![ManaType::Black],
                                target: PlayerActionTarget::SelfPlayer,
                            })),
                        ],
                        CardPhase::Ready,
                        CardType::Mana,
                        vec![],
                        vec![]
                    ),
                ],
            )
        ).await;
        game.add_player(Player::new("Player 2", 100, 5, vec![

            Card::new(
                "Stabber",
                "A stabbing creature",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::TapWithinPhases(vec![TurnPhase::DeclareAttackers]),
                        Arc::new(DeclareAttackerAction {}),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::ManualWithinPhases(vec![], vec![TurnPhase::DeclareBlockers]),
                        Arc::new(DeclareBlockerAction {}),
                    ),
                ],
                CardPhase::Charging(1),
                CardType::Creature,
                vec![Stat::new(StatType::Damage, 6), Stat::new(StatType::Defense, 1)],
                vec![ManaType::White, ManaType::Blue]
            ),

            Card::new(
                "Plains",
                "TAP: Adds 1 white mana to your pool",
                vec![
                    CardActionTrigger::new(ActionTriggerType::Tap, Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::White],
                        target: PlayerActionTarget::SelfPlayer,
                    })),
                ],
                CardPhase::Ready,
                CardType::Mana,
                vec![],
                vec![]
            ),

            Card::new(
                "Destroy",
                "Destroy target card",
                vec![
                            CardActionTrigger::new(ActionTriggerType::Instant,
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
                    CardActionTrigger::new(ActionTriggerType::Tap, Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Blue],
                        target: PlayerActionTarget::SelfPlayer,
                    })),
                ],
                CardPhase::Ready,
                CardType::Mana,
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
        game.play_card(player, 0, None).await.expect("oh no, no mana?");
        for _ in 0..25 {
            game.advance_turn().await;
        }

        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        // // Card::tap(&player.lock().await.cards_in_play[0], &mut game, player).await;
        println!("{}", player.lock().await.render(30, 10, 30).await);
        game.play_card(player, 0, Some(posion_target)).await.expect("oh no, no mana?");

        for _ in 0..15 {
            game.advance_turn().await;
        }
        // println!("{:?}", player.lock().await.cards_in_play[1].lock().await.triggers);
        let target_card = {
            let card = &player.lock().await.cards_in_play[1];
            Arc::clone(card)
        };

        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        game.play_card(player, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");
        for _ in 0..25 {
            game.advance_turn().await;
        }
        game.play_card(player2, 0, None).await.expect("oh no");
        game.tap_card(player2, 0, None).await.expect("unable to tap, no ready?");
        let target_card = {
            let card = &player.lock().await.cards_in_play[2];
            Arc::clone(card)
        };
        game.play_card(player2, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");

        for _ in 0..25 {
            game.advance_turn().await;
        }

        // play white mana
        game.play_card(player2, 0, None).await.expect("oh no");
        // tap blue mana
        game.tap_card(player2, 0, None).await.expect("unable to tap, no ready?");
        // tap white mana
        game.tap_card(player2, 1, None).await.expect("unable to tap, no ready?");
        // play creature
        game.play_card(player2, 0, None).await.expect("oh no");

        for _ in 0..22 {
            game.advance_turn().await;
        }
        println!("Declaring attackers....");
        game.activate_card_action(player2, 2, Some(EffectTarget::Player(Arc::clone(player)))).await.expect("hmmmmm?");
        for _ in 0..8 {
            game.advance_turn().await;
        }
        game.tap_card(player, 0, None).await.expect("unable to tap, no ready?");
        game.play_card(player, 0, None).await.expect("hmm");
        for _ in 0..12 {
            game.advance_turn().await;
        }
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
        // game.play_card(player2, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");
    }

}
