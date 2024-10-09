use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    thread::Thread,
    time::Duration,
};

use action::{
    Action, ActionTriggerType, AsyncClosureAction, CardAction, CardActionTarget, CardActionTrigger,
    CardActionWrapper, CardRequiredTarget, CombatDamageAction, DestroyTargetCAction, TriggerTarget,
};
use card::{Card, CardPhase, CardType};
use combat::Combat;
use effects::{EffectID, EffectManager, EffectTarget};
use mana::{ManaPool, ManaType};
use player::Player;
use redis::Pipeline;
use serde::{Deserialize, Serialize};
use specta::Type;
use stat::{StatManager, StatType, Stats};
use tokio::{
    select,
    sync::{broadcast, mpsc, Mutex, Notify, RwLock},
    time::{sleep, timeout, Instant},
};
use turn::{Turn, TurnPhase};
use ulid::Ulid;

use crate::lobby::{
    lobby::DeckSelector,
    manager::{AbilityDetails, ExecuteAbility, LobbyCommand, LobbyTurnMessage},
};

pub mod action;
pub mod card;
pub mod combat;
pub mod decks;
pub mod effects;
pub mod mana;
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
    PlayedCard,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct CardWithDetails {
    pub card: Card,
    pub abilities: Vec<AbilityDetails>,
}

impl CardWithDetails {
    async fn get_abilities(
        card: &Card,
        turn_phase: TurnPhase,
        in_play: bool,
        original_card_arc: Option<Arc<Mutex<Card>>>,
        game_arc: Option<Arc<Mutex<Game>>>,
    ) -> Vec<AbilityDetails> {
        if card.tapped {
            return vec![];
        }

        if card.current_phase != CardPhase::Ready {
            if turn_phase == TurnPhase::DeclareBlockers && card.card_type != CardType::Creature {
                return vec![];
            }
        }

        let mut abilities = vec![];
        for trigger in card.triggers.iter() {
            match &trigger.trigger_type {
                // action::ActionTriggerType::CardTapped => {
                //     return (trigger.card_required_target.clone(), ActionType::Tap)
                // }
                // action::ActionTriggerType::CardTappedWithinPhases(allowed_phases) => {
                //     if allowed_phases.contains(&turn_phase) {
                //         return (trigger.card_required_target.clone(), ActionType::Tap);
                //     }
                // }
                action::ActionTriggerType::CardPlayedFromHand => {
                    if &trigger.card_required_target != &CardRequiredTarget::None {
                        abilities.push(AbilityDetails {
                            id: trigger.id.clone(),
                            action_type: ActionType::PlayedCard,
                            mana_cost: vec![],
                            required_target: trigger.card_required_target.clone(),
                            description: "Play".to_string(),
                            show: true,
                            meets_requirements: true,
                        });
                    }
                }

                action::ActionTriggerType::Attached => {
                    if &turn_phase == &TurnPhase::Main {
                        abilities.push(AbilityDetails {
                            id: trigger.id.clone(),
                            action_type: ActionType::Attach,
                            mana_cost: vec![],
                            required_target: trigger.card_required_target.clone(),
                            description: "Attach".to_string(),
                            show: in_play,
                            meets_requirements: true,
                        });
                    }
                }

                action::ActionTriggerType::AbilityWithinPhases(
                    description,
                    required_mana,
                    required_with_phases,
                    required_tap,
                ) => {
                    let within_phase = required_with_phases
                        .as_ref()
                        .and_then(|phase| Some(phase.contains(&turn_phase)))
                        .unwrap_or(true);

                    if let Some(owner) = &card.owner {
                        let can_pay_mana = { owner.lock().await.can_pay_mana(required_mana).await };
                        if can_pay_mana && within_phase {
                            let mut meets_requirements = true;
                            if let Some(game_arc) = &game_arc {
                                if let Some(card) = &original_card_arc {
                                    meets_requirements = (&trigger.requirements)(
                                        Arc::clone(game_arc),
                                        Arc::clone(card),
                                    )
                                    .await;
                                }
                            }

                            abilities.push(AbilityDetails {
                                id: trigger.id.clone(),
                                mana_cost: required_mana.clone(),
                                required_target: trigger.card_required_target.clone(),
                                description: description.to_string(),
                                action_type: if *required_tap {
                                    ActionType::Tap
                                } else {
                                    ActionType::None
                                },
                                show: in_play || required_with_phases.is_none(),
                                meets_requirements,
                            });
                            // return (trigger.card_required_target.clone(), action_type);
                        }
                    }
                }
                x => {}
            }
        }

        abilities
    }

    pub async fn from_card_arc(
        card_arc: &Arc<Mutex<Card>>,
        turn_phase: TurnPhase,
        in_play: bool,
        game: &Arc<Mutex<Game>>,
    ) -> CardWithDetails {
        let card = card_arc.lock().await.clone();
        let abilities = CardWithDetails::get_abilities(
            &card,
            turn_phase,
            in_play,
            Some(Arc::clone(card_arc)),
            Some(Arc::clone(game)),
        )
        .await;
        CardWithDetails { card, abilities }
    }

    pub async fn from_card(card: Card, turn_phase: TurnPhase, in_play: bool) -> CardWithDetails {
        let abilities =
            CardWithDetails::get_abilities(&card, turn_phase, in_play, None, None).await;
        CardWithDetails { card, abilities }
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PriorityQueue {
    pub player_index: i32,
    pub time_left: i8,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PlayerState {
    pub public_info: PublicPlayerInfo,
    pub hand: Vec<CardWithDetails>,
    pub discard_pile: Vec<CardWithDetails>,
    pub status: PlayerStatus,
    pub is_leader: bool,
    pub player_index: i32,
    pub priority_queue: Option<PriorityQueue>,
    pub deck: DeckSelector,

    #[serde(skip_serializing, skip_deserializing)]
    pub player: Arc<Mutex<Player>>,
}
impl PlayerState {
    pub(crate) fn from_player(player: Arc<Mutex<Player>>, player_index: i32) -> PlayerState {
        PlayerState {
            public_info: PublicPlayerInfo {
                cards_in_play: vec![],
                spells: vec![],
                hand_size: 0,
                mana_pool: ManaPool::new(),
                health: 10,
            },
            deck: DeckSelector::Green,
            hand: vec![],
            discard_pile: vec![],
            status: PlayerStatus::Spectator,
            player,
            is_leader: false,
            player_index,
            priority_queue: None,
        }
    }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub enum FrontendPileName {
    Hand,
    Play,
    Spell,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub enum FrontendTarget {
    Card(FrontendCardTarget),
    Player(i32),
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct FrontendCardTarget {
    pub player_index: i32,
    pub pile: FrontendPileName,
    pub card_index: i32,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct Block {
    pub attacker: FrontendCardTarget,
    pub blocker: FrontendCardTarget,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct Attack {
    pub attacker: FrontendCardTarget,
    pub target: FrontendTarget,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, Default)]
pub struct PublicGameInfo {
    pub current_turn: Option<Turn>,
    pub priority_queue: Option<PriorityQueue>,
    pub attacks: Vec<Attack>,
    pub blocks: Vec<Block>,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PublicPlayerInfo {
    pub hand_size: i32,
    pub cards_in_play: Vec<CardWithDetails>,
    pub spells: Vec<CardWithDetails>,
    pub mana_pool: ManaPool,
    pub health: i8,
}

enum PriorityActionResult {
    NoAction,              // Player did nothing
    ActionRequiresRestart, // Player performed an action that requires restarting the priority loop
    Timeout,               // Player did not act in time
}

#[derive(Default, Deserialize, Serialize)]
pub struct Game {
    #[serde(skip_serializing, skip_deserializing)]
    pub players: Vec<Arc<Mutex<Player>>>,
    pub current_turn: Option<Turn>,
    pub turn_number: usize,
    #[serde(skip_serializing, skip_deserializing)]
    pub action_queue: Vec<Arc<dyn Action + Send + Sync>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub effect_manager: EffectManager,
    #[serde(skip_serializing, skip_deserializing)]
    pub event_stack: Vec<Arc<dyn Action + Send + Sync>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub combat: Combat,
    #[serde(skip_serializing, skip_deserializing)]
    pub current_priority_player: Option<(Arc<Mutex<Player>>, i8, ActionType)>,
    #[serde(skip_serializing, skip_deserializing)]
    pub broadcast_sender: Option<broadcast::Sender<Option<LobbyCommand>>>,
    pub turn_messages: Vec<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub abilities: HashMap<String, Ability>,
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
            .field("players", &self.players)
            .field("current_turn", &self.current_turn)
            .field("turn_number", &self.turn_number)
            .field("action_queue", &self.action_queue)
            .field("effect_manager", &self.effect_manager)
            .field("event_stack", &self.event_stack)
            .field("combat", &self.combat)
            .field("current_priority_player", &self.current_priority_player)
            .field("broadcast_sender", &self.broadcast_sender)
            .field("turn_messages", &self.turn_messages)
            .finish()
    }
}

#[derive(Clone)]
pub struct Ability {
    id: String,
    card_arc: Arc<Mutex<Card>>,
    mana_cost: Vec<ManaType>,
    target: CardRequiredTarget,
    description: String,
    ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
    action_type: ActionType,
}

impl fmt::Debug for Ability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ability")
            .field("id", &self.id)
            .field("card_arc", &self.card_arc)
            .field("mana_cost", &self.mana_cost)
            .field("target", &self.target)
            .finish()
    }
}
impl Ability {
    pub fn new(
        card_arc: Arc<Mutex<Card>>,
        mana_cost: Vec<ManaType>,
        target: CardRequiredTarget,
        ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
        description: String,
        action_type: ActionType,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            card_arc,
            mana_cost,
            target,
            ability,
            action_type,
            description,
        }
    }
}

impl Game {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);

        Self {
            players: vec![],
            current_turn: None,
            turn_number: 0,
            action_queue: vec![],
            effect_manager: EffectManager::new(),
            event_stack: vec![],
            combat: Combat::new(),
            current_priority_player: None,
            broadcast_sender: Some(sender),
            turn_messages: vec![],
            abilities: HashMap::new(),
        }
    }

    pub async fn ask_mandatory_player_ability(&mut self, ability: Ability) {
        self.abilities.insert(ability.id.clone(), ability.clone());
        if let Some(ref sender) = self.broadcast_sender {
            let player = ability
                .card_arc
                .lock()
                .await
                .owner
                .as_ref()
                .unwrap()
                .lock()
                .await
                .name
                .clone();
            let _ = sender.send(Some(LobbyCommand::MandatoryExecuteAbility(
                ExecuteAbility::new(
                    player,
                    CardWithDetails::from_card(
                        ability.card_arc.lock().await.clone(),
                        self.current_phase(),
                        true,
                    )
                    .await,
                    ability.action_type,
                    ability.mana_cost,
                    ability.target,
                    ability.description,
                    ability.id,
                    true,
                ),
            )));
        }
    }

    pub async fn request_player_ability(&mut self, ability: Ability) {
        self.abilities.insert(ability.id.clone(), ability.clone());
        if let Some(ref sender) = self.broadcast_sender {
            let player = ability
                .card_arc
                .lock()
                .await
                .owner
                .as_ref()
                .unwrap()
                .lock()
                .await
                .name
                .clone();
            let _ = sender.send(Some(LobbyCommand::AskExecuteAbility(ExecuteAbility::new(
                player,
                CardWithDetails::from_card(
                    ability.card_arc.lock().await.clone(),
                    self.current_phase(),
                    true,
                )
                .await,
                ability.action_type,
                ability.mana_cost,
                ability.target,
                ability.description,
                ability.id,
                true,
            ))));
        }
    }

    pub async fn has_tapped_creature_excluding(&self, cards: &Vec<Arc<Mutex<Card>>>) -> bool {
        for player in &self.players {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (index, card_arc) in cards_in_play.iter().enumerate() {
                if let Ok(card) = card_arc.try_lock() {
                    if card.tapped && card.card_type == CardType::Creature {
                        for check_against in cards {
                            if Arc::ptr_eq(check_against, card_arc) {
                                continue;
                            }

                            return true;
                        }
                        // } else {
                        //     println!(
                        //         "-----hmm skipping card player--- {:?} card {}",
                        //         player, index
                        //     );
                    }
                }
            }
        }

        false
    }

    pub async fn respond_player_ability(
        game_arc: Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        ability_id: String,
        response: bool,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let ability = {
            let game = game_arc.lock().await;
            game.abilities
                .get(&ability_id)
                .ok_or_else(|| "No ability with that id".to_string())?
                .clone()
        };

        let phase = {
            let game = game_arc.lock().await;
            game.current_phase()
        };
        let player = Arc::clone(player);

        if response {
            println!("Processing response for ability: {:?}", ability);
            if !ability.mana_cost.is_empty() {
                println!("it has a cost!");
                let cloned_ability_id = ability_id.clone();
                tokio::spawn(async move {
                    println!("Starting async task for ability...");
                    loop {
                        let current_phase = {
                            let game = game_arc.lock().await;
                            game.current_turn.as_ref().unwrap().phase
                        };

                        // Exit if we're no longer in the correct phase
                        if current_phase != phase {
                            println!("wrong phase, guess they did not really want to");
                            return;
                        }
                        println!("still in phase...");

                        // Check if the player can pay the mana cost
                        let can_pay_mana = player
                            .lock()
                            .await
                            .has_required_mana(&ability.mana_cost)
                            .await;

                        if can_pay_mana {
                            println!("can pay mana");
                            player.lock().await.pay_mana(&ability.mana_cost).await;
                            let mut game = game_arc.lock().await;
                            println!("executing");
                            game.execute_ability(cloned_ability_id, target).await.ok();
                            return;
                        }

                        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    }
                });
            } else {
                let mut game = game_arc.lock().await;
                game.execute_ability(ability_id, target).await?;
            }
        }

        Ok(())
    }

    pub async fn execute_ability(
        &mut self,
        ability_id: String,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let ability = self
            .abilities
            .remove(&ability_id)
            .ok_or("No ability with that id".to_string())?;

        let card_arc = ability.card_arc.clone();
        let action = (ability.ability)(card_arc.clone());
        self.add_to_stack(Arc::new(CardActionWrapper {
            card: card_arc,
            action,
            target,
        }));
        self.resolve_stack().await;

        Ok(())
    }

    pub async fn frontend_target_from_card(&self, arc: &Arc<Mutex<Card>>) -> FrontendCardTarget {
        for (player_index, player) in self.players.iter().enumerate() {
            for (card_index, card) in player.clone().lock().await.cards_in_hand.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Hand,
                        player_index: player_index as i32,
                    };
                }
            }
            for (card_index, card) in player.clone().lock().await.spells.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Spell,
                        player_index: player_index as i32,
                    };
                }
            }
            for (card_index, card) in player.clone().lock().await.cards_in_play.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Play,
                        player_index: player_index as i32,
                    };
                }
            }
        }

        FrontendCardTarget {
            card_index: 0,
            pile: FrontendPileName::Hand,
            player_index: 0,
        }
    }

    pub async fn frontend_target_from_effect_target(
        &self,
        target: &EffectTarget,
    ) -> FrontendTarget {
        match target {
            EffectTarget::Player(arc) => FrontendTarget::Player(0),
            EffectTarget::Card(arc) => {
                FrontendTarget::Card(self.frontend_target_from_card(arc).await)
            }
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
                    card.damage_dealt_to_players = 0;
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

        self.effect_manager
            .apply_effects(self.current_turn.clone().unwrap())
            .await;
    }

    pub async fn remove_references_to(&mut self, card: &Arc<Mutex<Card>>) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];

        for player_arc in self.players.clone() {
            let player = player_arc.lock().await;

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
                    self.detach_card(card_in_play_arc).await;
                }
            }
        }

        self.execute_actions(&mut actions).await;
    }

    pub async fn detach_card(&mut self, card_arc: &Arc<Mutex<Card>>) {
        let mut card = card_arc.lock().await;
        if let Some(attached_card) = card.attached.take() {
            println!("Detached card {} from \n\n{:?}", card.name, attached_card);
            self.effect_manager
                .remove_effects_by_source(card_arc, self.current_turn.clone().unwrap())
                .await;
        }
    }

    pub async fn destroy_card(&mut self, card: &Arc<Mutex<Card>>) {
        self.remove_references_to(card).await;
        let mut actions = vec![];
        for player in self.players.iter() {
            let cards_to_destroy = {
                let player_locked = player.lock().await;

                let mut cards_to_destroy = Vec::new();
                for (card_index, card_in_play) in player_locked.cards_in_play.iter().enumerate() {
                    if Arc::ptr_eq(card, card_in_play) {
                        cards_to_destroy.push(card_index);
                    }
                }

                cards_to_destroy
            };

            for &card_index in &cards_to_destroy {
                let mut player_locked = player.lock().await;
                actions.append(
                    &mut player_locked
                        .destroy_card_in_play(card_index, self.current_turn.as_ref().unwrap())
                        .await,
                );
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
        target
            .clone()
            .ok_or_else(|| "Choose a target".to_string())?;

        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked
                .attach_card(in_play_index, target, self)
                .await?
        };

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn activate_card_action(
        &mut self,
        player: &Arc<Mutex<Player>>,
        in_play_index: usize,
        target: Option<EffectTarget>,
        trigger_id: String,
    ) -> Result<(), String> {
        if let Some((current_player, _, action_taken)) = &mut self.current_priority_player {
            if !Arc::ptr_eq(&player, current_player) {
                return Err("Not your turn".to_string());
            } else {
                *action_taken = ActionType::Tap;
            }
        }
        let game_arc = Arc::new(Mutex::new(std::mem::take(self)));
        let mut actions = {
            Player::execute_action(
                Arc::clone(player),
                in_play_index,
                target,
                game_arc.clone(),
                trigger_id,
            )
            .await?
        };
        let mut game_unlocked = game_arc.lock().await;
        *self = std::mem::take(&mut *game_unlocked);

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn activate_card_action_old(
        &mut self,
        player: &Arc<Mutex<Player>>,
        in_play_index: usize,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        if let Some((current_player, _, action_taken)) = &mut self.current_priority_player {
            if !Arc::ptr_eq(&player, current_player) {
                return Err("Not your turn".to_string());
            } else {
                *action_taken = ActionType::Tap;
            }
        }
        let mut actions = {
            let mut player_locked = player.lock().await;
            player_locked
                .execute_action_old(in_play_index, target, self)
                .await?
        };

        self.execute_actions(&mut actions).await;

        Ok(())
    }

    pub async fn play_token(
        game_arc: &Arc<Mutex<Game>>, // Now passing the game Arc
        player_arc: &Arc<Mutex<Player>>,
        mut token: Card,
    ) -> Result<Arc<Mutex<Card>>, String> {
        token.owner = Some(Arc::clone(player_arc));
        let card = Arc::new(Mutex::new(token));

        let index = {
            let mut player = player_arc.lock().await;
            player.cards_in_hand.push(card.clone());
            player.cards_in_hand.len() - 1
        };

        let mut game = game_arc.lock().await; // Lock the game for write access
        let result = game.execute_card(player_arc, index, None).await?;
        game.resolve_stack().await;

        // Now pass the game Arc to process the action queue
        // Game::process_action_queue(game_arc.clone(), result.clone()).await;

        Ok(result)
    }

    pub async fn play_card(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        if let Some((current_player, _, action_taken)) = &mut self.current_priority_player {
            if !Arc::ptr_eq(&player, current_player) {
                return Err("Not your turn".to_string());
            } else {
                *action_taken = ActionType::PlayedCard;
            }
        }

        Ok(self.execute_card(player, index, target).await?)
    }

    async fn execute_card(
        &mut self,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let (action, card) = {
            Player::play_card(
                player,
                index,
                target.clone(),
                self.current_turn.clone().unwrap(),
            )
            .await?
        };

        self.add_to_stack(action);
        self.effect_manager
            .apply_effects(self.current_turn.clone().unwrap())
            .await;

        Ok(card)
    }

    pub async fn next_priority_queue(&mut self) {}

    pub fn send_command(&mut self, command: LobbyCommand) {
        if let Some(ref sender) = self.broadcast_sender {
            let _ = sender.send(Some(command));
        }
    }

    pub fn debug(&mut self, message: &str) {
        println!("DEBUG: {}", message);
        if let Some(ref sender) = self.broadcast_sender {
            let _ = sender.send(Some(LobbyCommand::DebugMessage(message.to_string())));
        }
    }

    pub fn add_turn_message(&mut self, message: String) {
        self.turn_messages.push(message);
        self.messages_updated();
    }

    pub async fn collect_card_played_actions(
        &self,
        card_arc: &Arc<Mutex<Card>>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let owner = card_arc.lock().await.owner.clone();
        if let Some(owner) = &owner {
            for player in &self.players {
                let cards = player.lock().await.cards_in_play.clone();

                for card in &cards {
                    let triggers = card.lock().await.triggers.clone();
                    let target = Some(EffectTarget::Card(Arc::clone(card_arc)));
                    for trigger in triggers {
                        if &trigger.trigger_type == &ActionTriggerType::CardPlayedFromHand
                            && Arc::ptr_eq(card, card_arc)
                        {
                            actions.push(Arc::new(CardActionWrapper {
                                action: trigger.action.clone(),
                                card: Arc::clone(card),
                                target: target.clone(),
                            }));
                        }

                        if let ActionTriggerType::CreatureTypeCardPlayed(
                            trigger_target,
                            creature_type,
                        ) = &trigger.trigger_type
                        {
                            match trigger_target {
                                TriggerTarget::Target => todo!(),
                                TriggerTarget::Owner => {
                                    if Arc::ptr_eq(card, card_arc) {
                                        continue;
                                    }
                                    let card_creature_type = { card.lock().await.creature_type };
                                    if card_creature_type == Some(*creature_type) {
                                        if self
                                            .current_turn
                                            .as_ref()
                                            .and_then(|x| {
                                                Some(Arc::ptr_eq(&x.current_player, owner))
                                            })
                                            .unwrap_or(false)
                                        {
                                            actions.push(Arc::new(CardActionWrapper {
                                                action: trigger.action.clone(),
                                                card: Arc::clone(card),
                                                target: target.clone(),
                                            }));
                                        }
                                    }
                                }
                                TriggerTarget::Any => todo!(),
                            }
                        }
                        if let ActionTriggerType::OtherCardPlayed(trigger_target) =
                            &trigger.trigger_type
                        {
                            match trigger_target {
                                TriggerTarget::Target => todo!(),
                                TriggerTarget::Owner => {
                                    if Arc::ptr_eq(card, card_arc) {
                                        continue;
                                    }
                                    if self
                                        .current_turn
                                        .as_ref()
                                        .and_then(|x| Some(Arc::ptr_eq(&x.current_player, owner)))
                                        .unwrap_or(false)
                                    {
                                        actions.push(Arc::new(CardActionWrapper {
                                            action: trigger.action.clone(),
                                            card: Arc::clone(card),
                                            target: target.clone(),
                                        }));
                                    }
                                }
                                TriggerTarget::Any => todo!(),
                            }
                        }
                    }
                }
            }
        }

        actions
    }

    pub async fn collect_actions_for_phase(&mut self) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions = Vec::new();

        for (player_index, player) in self.players.iter().enumerate() {
            let mut a = Player::collection_actions_for_phase(
                Arc::clone(player),
                player_index,
                self.current_turn.clone().unwrap(),
            )
            .await;
            actions.append(&mut a);

            for card_rc in &player.lock().await.cards_in_play {
                let collected_actions: Vec<Arc<dyn Action + Send + Sync>> =
                    Card::collect_phase_based_actions(
                        card_rc,
                        &self.current_turn.clone().unwrap(),
                        action::ActionTriggerType::PhaseStarted(
                            vec![self.current_phase()],
                            TriggerTarget::Any,
                        ),
                    )
                    .await;
                actions.extend(collected_actions);

                let has_effects = self.effect_manager.has_effects(card_rc).await;
                if card_rc.lock().await.is_useless(has_effects) {
                    println!("card is considered useless, let's get rid of it");
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(DestroyTargetCAction {}),
                        card: Arc::clone(card_rc),
                        target: None,
                    }));
                }
            }
        }

        actions
    }

    pub async fn collect_omnipresent_actions(&mut self) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for player in &self.players {
            let cards = player.lock().await.cards_in_play.clone();

            for card in &cards {
                let triggers = card.lock().await.triggers.clone();
                let target = Some(EffectTarget::Card(Arc::clone(&card)));
                for trigger in triggers {
                    match trigger.trigger_type {
                        ActionTriggerType::Continuous => {
                            actions.push(Arc::new(CardActionWrapper {
                                action: trigger.action,
                                card: Arc::clone(card),
                                target: target.clone(),
                            }))
                        }
                        _ => {}
                    }
                }
            }
        }

        actions
    }

    pub async fn execute_actions(&mut self, actions: &mut Vec<Arc<dyn Action + Send + Sync>>) {
        let actions_to_execute = std::mem::take(actions);

        for action in actions_to_execute {
            println!("Applying action {:?}", action);
            action.apply(self).await;
        }
        let actions_to_execute = self.collect_omnipresent_actions().await;
        for action in actions_to_execute {
            println!("Applying action {:?}", action);
            action.apply(self).await;
        }

        self.effect_manager
            .apply_effects(self.current_turn.clone().unwrap())
            .await;
    }

    pub fn reset_turn_messages(&mut self) {
        self.turn_messages = vec![];
        self.messages_updated();
    }

    pub async fn start_turn(&mut self, player_index: usize) {
        self.reset_turn_messages();

        self.current_turn = Some(Turn::new(
            self.players[player_index].clone(),
            player_index,
            self.turn_number,
        ));
        self.turn_number += 1;

        self.reset_creature_damage().await;

        for player in &self.players {
            let mut player = player.lock().await;
            player.health_at_start_of_round = player.stat_manager.get_stat_value(StatType::Health);
            player.triggers_played_this_turn = HashSet::new();
        }

        let player_arc = self.players[player_index].clone();
        {
            let mut player = player_arc.lock().await;
            player.advance_card_phases().await;
        }
    }
    pub async fn print(&self) {
        let player = self.players[self.current_turn.clone().unwrap().current_player_index as usize]
            .lock()
            .await;
        println!(
            "{}'s turn: ------\n{}",
            player.name,
            player.render(30, 10, 30).await
        );
    }

    pub async fn handle_deaths(&mut self) {
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

        self.players = alive_players;
    }

    pub async fn execute_player_action(
        &mut self,
        player_arc: Arc<Mutex<Player>>,
        action: Arc<dyn Action + Send + Sync>,
    ) -> Result<(), String> {
        action.apply(self).await;

        Ok(())
    }

    pub fn get_players_in_priority_order(
        &self,
        starting_player: &Arc<Mutex<Player>>,
    ) -> Vec<Arc<Mutex<Player>>> {
        let mut players_in_order = Vec::new();

        // Ensure there are players in the game
        if !self.players.is_empty() {
            // Find the index of the starting player
            if let Some(starting_index) = self
                .players
                .iter()
                .position(|p| Arc::ptr_eq(p, starting_player))
            {
                let num_players = self.players.len();

                // Start from the next player after the starting_player
                // Loop through all players except the starting_player
                for i in 1..num_players {
                    let index = (starting_index + i) % num_players;
                    players_in_order.push(self.players[index].clone());
                }
                // This will exclude the starting_player from the list
            }
        }

        players_in_order
    }

    pub async fn wait_for_player_action_async(
        game_arc: Arc<Mutex<Game>>,
        initial_time_limit: i8,
    ) -> PriorityActionResult {
        let sleep_duration = Duration::from_millis(100);
        let mut deadline = Instant::now() + Duration::from_secs(initial_time_limit as u64);
        let mut time_since_last_notification = Duration::from_secs(0);
        let current_player = Arc::clone(
            &game_arc
                .lock()
                .await
                .current_priority_player
                .clone()
                .unwrap()
                .0,
        );

        loop {
            if Instant::now() >= deadline {
                return PriorityActionResult::Timeout;
            }

            // Remove the check for current_priority_player change
            // The player remains the same during their priority turn

            let action_performed = {
                let mut game = game_arc.lock().await;
                match game.performed_action() {
                    ActionType::PlayedCard => {
                        println!("Player performed an action requiring priority loop restart.");
                        // Reset the action performed flag
                        // {
                        //     let mut game = game_arc.write().await;
                        //     if let Some((_, tl, _)) = &mut game.current_priority_player {
                        //         *tl = time_left as i8;
                        //     }
                        // } // Write lock released here

                        // game.reset_performed_action();
                        return PriorityActionResult::ActionRequiresRestart;
                    }
                    ActionType::None => {
                        // No action performed
                    }
                    x => {
                        println!("Player performed an action {:?}, resetting timer.", x);
                        if let Some((_, _, action)) = &mut game.current_priority_player {
                            *action = ActionType::None;
                        }

                        deadline = Instant::now() + Duration::from_secs(15);
                    }
                }
            };

            let time_left = deadline.saturating_duration_since(Instant::now()).as_secs();
            {
                let mut game = game_arc.lock().await;
                if let Some((_, tl, _)) = &mut game.current_priority_player {
                    *tl = time_left as i8;
                }
            }

            time_since_last_notification += sleep_duration;

            if time_since_last_notification >= Duration::from_secs(1) {
                time_since_last_notification = Duration::from_secs(0);

                let sender = {
                    let game = game_arc.lock().await;
                    game.broadcast_sender.clone()
                };

                if let Some(sender) = sender {
                    let _ = sender.send(None);
                }
            }

            sleep(sleep_duration).await;
        }
    }

    pub fn performed_action(&self) -> ActionType {
        if let Some((_, _, action)) = &self.current_priority_player {
            return action.clone();
        }
        ActionType::None
    }

    pub async fn priority_loop(game_arc: Arc<Mutex<Game>>, source_card_arc: Arc<Mutex<Card>>) {
        let mut players_in_order = {
            let mut game = game_arc.lock().await;
            game.debug("Entering priority loop");
            game.get_players_in_priority_order(&source_card_arc.lock().await.owner.clone().unwrap())
        };

        loop {
            let num_players = players_in_order.len();
            let mut passed_players = vec![false; num_players];
            let mut all_passed = true;

            for (i, player_arc) in players_in_order.iter().enumerate() {
                if passed_players[i] {
                    continue;
                }

                {
                    let mut player = player_arc.lock().await;
                    player.priority_turn_start().await;
                    println!("Player {}'s priority turn has started.", player.name);
                }
                let time_limit = if i == 0 { 3 } else { 3 };

                {
                    let mut game = game_arc.lock().await;
                    game.current_priority_player =
                        Some((player_arc.clone(), time_limit.clone(), ActionType::None));

                    if let Some(ref sender) = game.broadcast_sender {
                        let _ = sender.send(None);
                    }
                }

                let game_arc_clone = Arc::clone(&game_arc);
                let result = Game::wait_for_player_action_async(game_arc_clone, time_limit).await;

                match result {
                    PriorityActionResult::ActionRequiresRestart => {
                        println!("Player performed an action requiring priority loop restart.");
                        // Restart the priority loop from the player who performed the action
                        players_in_order = {
                            let game = game_arc.lock().await;
                            game.get_players_in_priority_order(player_arc)
                        };
                        // Reset passed players
                        passed_players = vec![false; players_in_order.len()];
                        // Start the loop again
                        break;
                    }
                    PriorityActionResult::Timeout => {
                        println!(
                            "{} did not act in time, passing.",
                            player_arc.lock().await.name
                        );
                        passed_players[i] = true;
                    }
                    PriorityActionResult::NoAction => {
                        println!("{} passed without action.", player_arc.lock().await.name);
                        passed_players[i] = true;
                    }
                }

                {
                    let mut player = player_arc.lock().await;
                    player.priority_turn_end().await;
                }
            }

            if passed_players.iter().all(|&passed| passed) {
                println!("All players have passed. Exiting priority loop.");

                {
                    let game = game_arc.lock().await;
                    if let Some(ref sender) = game.broadcast_sender {
                        let _ = sender.send(None);
                    }
                }

                break;
            }
        }

        {
            let mut game = game_arc.lock().await;
            game.current_priority_player = None;
        }
    }

    pub async fn process_action_queue(game_arc: Arc<Mutex<Game>>, card_arc: Arc<Mutex<Card>>) {
        let card = { card_arc.lock().await.clone() };

        if card.card_type.is_spell() {
            {
                let mut game = game_arc.lock().await;
                game.add_turn_message(format!(
                    "{} is casting {}",
                    card.owner.unwrap().lock().await.name,
                    card.name
                ));
            }

            Self::priority_loop(Arc::clone(&game_arc), card_arc).await;
        }

        let mut game = game_arc.lock().await;

        game.resolve_stack().await;

        if let Some(ref sender) = game.broadcast_sender {
            let _ = sender.send(None);
        }
    }

    pub fn messages_updated(&self) {
        if let Some(ref sender) = self.broadcast_sender {
            let _ = sender.send(Some(LobbyCommand::TurnMessages(LobbyTurnMessage {
                messages: self.turn_messages.clone(),
            })));
        }
    }

    pub async fn advance_turn(&mut self) {
        if let Some(ref mut turn) = self.current_turn {
            if let Some((current_player, _, action_taken)) = &mut self.current_priority_player {
                if !Arc::ptr_eq(&turn.current_player, current_player) {
                    println!("cannot advance turn while waiting for priority queue.");
                    return;
                }
            }

            turn.next_phase();
            if turn.phase == TurnPhase::Untap {
                let next_player_index = (turn.current_player_index + 1) % self.players.len() as i32;
                println!("advancing player? {}", next_player_index);
                self.start_turn(next_player_index as usize).await;
            }

            let mut actions = self.collect_actions_for_phase().await;
            self.execute_actions(&mut actions).await;

            println!(
                ":: TURN ADVANCED :: {:?} effects: {:?}",
                self.current_turn.clone().unwrap().phase,
                ""
            );
        }
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
