use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::Arc,
};

use action::{Action, TriggerTarget};
use card::Card;
use effects::EffectTarget;
use player::Player;
use serde::{Deserialize, Serialize};
use specta::Type;
use stat::Stats;
use tokio::sync::Mutex;
use turn::{Turn, TurnPhase};

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
}

impl Game {
    pub fn new() -> Self {
        Self {
            players: vec![],
            current_turn: None,
            turn_number: 0,
            action_queue: vec![],
        }
    }

    // pub async fn destroy_card(&mut self, card: &Arc<Mutex<Card>>) {
    //     for player in self.players.iter() {
    //         println!("{:?}", player);
    //         // let player = Arc::clone(player);
    //         // let mut player = player.lock().await;

    //         // // First, collect the indices of cards that should be destroyed
    //         // let mut cards_to_destroy = Vec::new();

    //         // for (card_index, card_in_play) in player.cards_in_play.iter().enumerate() {
    //         //     if Arc::ptr_eq(card, card_in_play) {
    //         //         println!("found it");
    //         //         cards_to_destroy.push(card_index);
    //         //     }
    //         // }

    //         // // Now, destroy the cards
    //         // // Reverse the list of indices so that removing cards doesn't mess up the order
    //         // for card_index in cards_to_destroy.iter().rev() {
    //         //     player.destroy_card_in_play(*card_index);
    //         // }
    //     }
    // }
    pub async fn destroy_card(&mut self, card: &Arc<Mutex<Card>>) {
        // Iterate over the players
        for player in self.players.iter() {
            // println!("{:?}", player);
            let cards_to_destroy = {
                // Lock the player to identify cards that need to be destroyed
                let player_locked = player.lock().await;

                // Collect indices of cards that should be destroyed
                let mut cards_to_destroy = Vec::new();
                for (card_index, card_in_play) in player_locked.cards_in_play.iter().enumerate() {
                    if Arc::ptr_eq(card, card_in_play) {
                        cards_to_destroy.push(card_index);
                    }
                }

                cards_to_destroy // Return the indices of cards to destroy
            }; // Player lock is released here

            // Now destroy the cards after the lock is released
            for &card_index in cards_to_destroy.iter().rev() {
                let mut player_locked = player.lock().await;
                player_locked.destroy_card_in_play(card_index);
            }
        }
    }


    pub async fn add_player(&mut self,  player: Player) {
        let owner = Arc::new(Mutex::new(player));

        let player = owner.lock().await;
        player.deck.set_owner(&owner).await;
        self.players.push(Arc::clone(&owner));
    }

    pub async fn play_card(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let mut actions = {
            // Lock the player to play the card and collect actions
            let mut player_locked = player.lock().await;
            player_locked.play_card(index, target, self).await?
        }; // Player lock is released here

        // Execute the actions after unlocking the player
        self.execute_actions(&mut actions).await;

        Ok(())
    }


    pub async fn collect_actions_for_phase(&mut self) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions = Vec::new();

        for (player_index,player) in self.players.iter().enumerate() {
            let mut a = Player::collection_actions_for_phase(Arc::clone(player), player_index, self.current_turn.clone().unwrap()).await;
            actions.append(&mut a);


            for card_rc in &player.lock().await.cards_in_play {
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
        self.current_turn = Some(Turn::new(self.players[player_index].clone(), player_index, self.turn_number));
        self.turn_number += 1;

        let player = self.players[player_index].lock().await;
        println!("{}'s turn: ------\n{}", player.name, player.render(30, 10, 30).await);
    }


    pub async fn advance_turn(&mut self) {
        if let Some(ref mut turn) = self.current_turn {




            let mut actions = self.collect_actions_for_phase().await;


            self.execute_actions(&mut actions).await;


            if let Some(ref mut turn) = self.current_turn {
                turn.next_phase();


                if turn.phase == TurnPhase::Untap {
                    let next_player_index = (turn.current_player_index + 1) % self.players.len();
                    self.start_turn(next_player_index).await;
                }
            }
        }

        // println!("{:?}", self.current_turn.clone().unwrap().phase);
    }
}
mod test {
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use tokio::sync::Mutex;

    use crate::game::{action::{generate_mana::GenerateManaAction, ActionTriggerType, CardActionTrigger, CardActionWrapper, DestroyCardAction, PlayerActionTarget, PlayerActionTrigger},  effects::EffectTarget, mana::ManaType};

    use super::{
        action::{CardActionTarget, CardDamageAction, TriggerTarget}, card::{Card, CardType, CardPhase}, effects::{
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
                    Card::new(
                        "Poison",
                        "Applies 2 damage for the next 3 turns after 1 turn.",
                        vec![
                            CardActionTrigger::new(ActionTriggerType::PhaseBased(vec![TurnPhase::Upkeep], TriggerTarget::Owner),
                                Arc::new(CardDamageAction {
                                    target: CardActionTarget::Target
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
                            CardActionTrigger::new(ActionTriggerType::Manual, Arc::new(GenerateManaAction {
                                mana_to_add: vec![ManaType::Black],
                                target: PlayerActionTarget::SelfPlayer,
                            })),
                        ],
                        CardPhase::Ready,
                        CardType::Mana(ManaType::Black),
                        vec![],
                        vec![]
                    ),
                ],
            )
        ).await;
        game.add_player(Player::new("Player 2", 100, 5, vec![
            Card::new(
                "Destroy",
                "Destroy target card",
                vec![
                            CardActionTrigger::new(ActionTriggerType::Instant,
                                Arc::new(DestroyCardAction {})
                            )
                ],
                CardPhase::Charging(1),
                CardType::Instant,
                vec![Stat::new(StatType::Damage, 1)],
                vec![ManaType::Blue]
            ),


            Card::new(
                "Swamp",
                "TAP: Adds 1 Swamp mana to your pool",
                vec![
                    CardActionTrigger::new(ActionTriggerType::Manual, Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Blue],
                        target: PlayerActionTarget::SelfPlayer,
                    })),
                ],
                CardPhase::Ready,
                CardType::Mana(ManaType::Blue),
                vec![],
                vec![]
            ),
        ])).await;



        game.start_turn(0).await;
        for _ in 0..17 {
            game.advance_turn().await;
        }

        let player = &game.players[0].clone();
        let player2 = &game.players[1].clone();
        let posion_target = EffectTarget::Player(Arc::clone(player2));
        game.play_card(player, 0, None).await.expect("oh no, no mana?");
        for _ in 0..10 {
            game.advance_turn().await;
        }

        Player::tap_card(player, 0, &mut game).await.expect("unable to tap, no ready?");
        // Card::tap(&player.lock().await.cards_in_play[0], &mut game, player).await;
        println!("{}", player.lock().await.render(30, 10, 30).await);
        game.play_card(player, 0, Some(posion_target)).await.expect("oh no, no mana?");

        for _ in 0..18 {
            game.advance_turn().await;
        }
        game.play_card(player2, 0, None).await.expect("oh no");
        Player::tap_card(player2, 0, &mut game).await.expect("unable to tap, no ready?");
        let target_card = {
            let card = &player.lock().await.cards_in_play[1];
            Arc::clone(card)
        };
        game.play_card(player2, 0, Some(EffectTarget::Card(target_card))).await.expect("oh no");

        for _ in 0..7 {
            game.advance_turn().await;
        }
    }

}
