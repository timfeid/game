pub mod generate_mana;
use async_trait::async_trait;
use std::{
    fmt::{self, Debug},
    sync::Arc,
};
use tokio::sync::Mutex;

use super::{
    card::Card,
    effects::{Effect, EffectTarget},
    player::Player,
    stat::{StatType, Stats},
    turn::{Turn, TurnPhase},
    Game,
};
use crate::game::stat::Stat;

#[derive(Debug, Clone)]
pub struct PlayerActionTrigger {
    pub trigger_type: ActionTriggerType,
    pub action: Arc<dyn PlayerAction + Send + Sync>,
}

impl PlayerActionTrigger {
    pub fn new(
        trigger_type: ActionTriggerType,
        action: Arc<dyn PlayerAction + Send + Sync>,
    ) -> Self {
        Self {
            trigger_type,
            action,
        }
    }

    pub async fn trigger(&self, game: &mut Game, index: usize) {
        if let ActionTriggerType::Manual = self.trigger_type {
            self.action.apply(game, index).await;
        }
    }

    pub(crate) async fn applies_in_phase(&self, turn: Turn, player: Arc<Mutex<Player>>) -> bool {
        match &self.trigger_type {
            ActionTriggerType::Manual => false,
            ActionTriggerType::PhaseBased(phases, trigger_target) => match trigger_target {
                TriggerTarget::Owner => {
                    phases.contains(&turn.phase) && Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Target => {
                    phases.contains(&turn.phase) && !Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Any => phases.contains(&turn.phase) && true,
            },
            ActionTriggerType::Instant => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardActionTrigger {
    pub trigger_type: ActionTriggerType,
    pub action: Arc<dyn CardAction + Send + Sync>,
}

impl CardActionTrigger {
    pub fn new(trigger_type: ActionTriggerType, action: Arc<dyn CardAction + Send + Sync>) -> Self {
        Self {
            trigger_type,
            action,
        }
    }

    pub async fn trigger(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        if let ActionTriggerType::Manual = self.trigger_type {
            self.action.apply(game, card).await;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTarget {
    Owner,
    Target,
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardActionTarget {
    This,
    Owner,
    Target,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerActionTarget {
    SelfPlayer,
    Opponent,
}

// A generic async action trait that allows both card and player actions to be treated similarly
#[async_trait]
pub trait Action: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game);
}
#[derive(Debug, Clone)]
pub enum CardActionType {
    Manual,     // For manually triggered actions (like tapping)
    PhaseBased, // For actions triggered during specific phases
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionTriggerType {
    Manual,                                    // Manually triggered, e.g., tapping a card
    PhaseBased(Vec<TurnPhase>, TriggerTarget), // Triggered during specific phases
    Instant,
}

#[async_trait::async_trait]
pub trait CardAction: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>);
    // fn action_type(&self) -> CardActionType;
    // async fn applies_in_phase(&self, phase: Turn, owner: Arc<Mutex<Player>>) -> bool;
}

// Implement `Action` for `CardAction` to make them compatible with the action queue
#[derive(Clone)]
pub struct CardActionWrapper {
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card: Arc<Mutex<Card>>,
    pub owner: Arc<Mutex<Player>>,
}

impl Debug for CardActionWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardActionWrapper")
            .field("action", &self.action)
            .finish()
    }
}

#[async_trait]
impl Action for CardActionWrapper {
    async fn apply(&self, game: &mut Game) {
        let card = Arc::clone(&self.card);
        let owner = Arc::clone(&self.owner);
        self.action.apply(game, card).await;
    }
}

// Trait for player-specific async actions
#[async_trait]
pub trait PlayerAction: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game, player_index: usize);
}

// Implement `Action` for `PlayerAction` to make them compatible with the action queue
#[derive(Clone)]
pub struct PlayerActionWrapper {
    pub action: Arc<dyn PlayerAction + Send + Sync>,
    pub player_index: usize,
}

impl Debug for PlayerActionWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayerActionWrapper")
            .field("action", &self.action)
            .finish()
    }
}

#[async_trait]
impl Action for PlayerActionWrapper {
    async fn apply(&self, game: &mut Game) {
        self.action.apply(game, self.player_index).await;
    }
}

// Example player action: draw a card
#[derive(Debug, Clone)]
pub struct DrawCardAction {
    pub target: PlayerActionTarget, // The target of the action
}

#[async_trait]
impl PlayerAction for DrawCardAction {
    async fn apply(&self, game: &mut Game, player_index: usize) {
        match self.target {
            PlayerActionTarget::SelfPlayer => {
                game.players[player_index].lock().await.draw_card(); // Draw a card for the current player
            }
            PlayerActionTarget::Opponent => {
                let opponent_index = (player_index + 1) % game.players.len();
                game.players[opponent_index].lock().await.draw_card();
            }
        }
    }
}

// Example card action: apply damage
#[derive(Debug, Clone)]
pub struct CardDamageAction {
    pub target: CardActionTarget,
}

#[async_trait]
impl CardAction for CardDamageAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        let card = card.lock().await;
        let target = &card.target;

        match self.target {
            CardActionTarget::Target => {
                if let Some(EffectTarget::Player(stats)) = target {
                    let mut stats = stats.lock().await;
                    let offense = card.get_stat_value(StatType::Damage);
                    let defense = stats.get_stat_value(StatType::Defense);
                    let total = offense - defense;
                    println!("do damage {}", total);
                    stats.add_stat(Stat::new(StatType::Health, -1 * total));
                }
            }
            _ => todo!(),
        }
    }

    // async fn applies_in_phase(&self, turn: Turn, owner: Arc<Mutex<Player>>) -> bool {
    //     println!("checking for cards on turn {:?}", turn);
    //     if let Some(phases) = &self.phases {
    //         if phases.contains(&turn.phase) {
    //             match self.on_turn {
    //                 CardTurnTarget::Owner => Arc::ptr_eq(&owner, &turn.current_player),
    //                 CardTurnTarget::Target => todo!(),
    //                 CardTurnTarget::Any => true,
    //             }
    //         } else {
    //             false
    //         } // Check if the action applies in the current phase
    //     } else {
    //         false
    //     }
    // }
}

#[derive(Debug, Clone)]
pub struct DestroyCardAction {}

#[async_trait]
impl CardAction for DestroyCardAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        let card = card.lock().await;
        let target = &card.target;

        match target {
            Some(EffectTarget::Card(target_card)) => {
                game.destroy_card(target_card).await;

                // You now have access to the target card
                // println!("Destroying card {:?}", target_card);

                // Implement the logic to remove the card from the game
                // For example, remove it from the player's hand or the game board
                // self.destroy_target_card(game, target_card.clone()).await;
            }
            Some(EffectTarget::Player(_)) => {
                // Handle cases where the target is a player if applicable
                println!("Cannot destroy a player");
            }
            None => {
                println!("No target to destroy");
            }
        }
    }
}
