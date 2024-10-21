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
    Action, ActionTriggerType, AsyncClosureAction, BlankAction, CardAction, CardActionTarget,
    CardActionTrigger, CardActionWrapper, CardRequiredTarget, ChooseFromSelectionAction,
    CombatDamageAction, DestroyTargetCAction, LifeLinkAction, PlayCardAction, TriggerTarget,
};
use card::{Card, CardPhase, CardType};
use combat::Combat;
use effects::{EffectID, EffectManager, EffectTarget};
use mana::{ManaPool, ManaType};
use player::Player;
use rand::seq::index;
use redis::Pipeline;
use serde::{Deserialize, Serialize};
use specta::Type;
use stat::{CardStatChangeListener, StatManager, StatType, Stats};
use tokio::{
    select,
    sync::{broadcast, mpsc, Mutex, MutexGuard, Notify, RwLock},
    time::{sleep, timeout, Instant},
};
use turn::{Turn, TurnPhase};
use ulid::Ulid;

use crate::lobby::{
    lobby::DeckSelector,
    manager::{
        AbilityDetails, CardSelectionDetails, ExecuteAbility, LobbyCommand, LobbyTurnMessage,
    },
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
    pub code: String,
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
    pub frontend_target: FrontendCardTarget,
}

impl CardWithDetails {
    async fn get_abilities(
        card: &Card,
        turn_phase: TurnPhase,
        in_play: bool,
        original_card_arc: Option<Arc<Mutex<Card>>>,
        game_arc: Option<Arc<Mutex<Game>>>,
    ) -> Vec<AbilityDetails> {
        let mut abilities = vec![];
        if card.current_phase == CardPhase::Exiled {
            return abilities;
        }
        let mut added_play_from_hand_action = false;

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
                action::ActionTriggerType::CardPlayedFromHand(restrictions) => {
                    let mut is_owner = !in_play;

                    if let Some(game) = &game_arc {
                        is_owner = Arc::ptr_eq(
                            &card.owner.clone().unwrap(),
                            &game
                                .lock()
                                .await
                                .current_turn
                                .as_ref()
                                .unwrap()
                                .current_player,
                        );
                    }
                    let within_phase = restrictions
                        .as_ref()
                        .and_then(|(phase, trigger_target)| {
                            Some(
                                phase.contains(&turn_phase)
                                    && match trigger_target {
                                        TriggerTarget::Owner => is_owner,
                                        TriggerTarget::Opponent => !is_owner,
                                        TriggerTarget::Any => true,
                                    },
                            )
                        })
                        .unwrap_or(true);

                    let mut meets_requirements_except_mana = within_phase && !in_play;
                    if let Some(game_arc) = &game_arc {
                        if let Some(card) = &original_card_arc {
                            meets_requirements_except_mana = meets_requirements_except_mana
                                && (&trigger.requirements)(
                                    Arc::clone(game_arc),
                                    Arc::clone(card),
                                    trigger.id.clone(),
                                )
                                .await;
                        }
                    }
                    let mut meets_mana_requirements = false;
                    let mut can_pay_mana = false;
                    let mut player_id = None;

                    if let Some(owner) = &card.owner {
                        meets_mana_requirements =
                            owner.lock().await.has_required_mana(&card.cost).await;
                        can_pay_mana = owner.lock().await.can_pay_mana(&card.cost).await;
                        player_id = Some(owner.lock().await.name.clone());
                    }

                    added_play_from_hand_action = true;
                    abilities.push(AbilityDetails {
                        id: "play_card".to_string(),
                        action_type: ActionType::PlayedCard,
                        mana_cost: vec![],
                        required_target: trigger.card_required_target.clone(),
                        description: "Play".to_string(),
                        show: false,
                        meets_requirements_except_mana,
                        meets_mana_requirements,
                        can_pay_mana,
                        owner_player_id: player_id,
                    });
                }

                action::ActionTriggerType::Attached => {
                    let mut player_id = None;

                    if let Some(owner) = &card.owner {
                        player_id = Some(owner.lock().await.name.clone());
                    }
                    if &turn_phase == &TurnPhase::Main {
                        abilities.push(AbilityDetails {
                            id: trigger.id.clone(),
                            action_type: ActionType::Attach,
                            mana_cost: vec![],
                            required_target: trigger.card_required_target.clone(),
                            description: "Attach".to_string(),
                            show: in_play,
                            meets_requirements_except_mana: in_play,
                            meets_mana_requirements: true,
                            can_pay_mana: true,
                            owner_player_id: player_id.clone(),
                        });
                    }
                }

                action::ActionTriggerType::AbilityWithinPhases(
                    description,
                    required_mana,
                    phase_restrictions,
                    required_tap,
                ) => {
                    let mut is_owner = false;
                    if let Some(game) = &game_arc {
                        is_owner = Arc::ptr_eq(
                            &card.owner.clone().unwrap(),
                            &game
                                .lock()
                                .await
                                .current_turn
                                .as_ref()
                                .unwrap()
                                .current_player,
                        );
                    }
                    let main_phase_restriction = phase_restrictions
                        .as_ref()
                        .and_then(|(phase, trigger_target)| {
                            Some(
                                phase.contains(&TurnPhase::Main)
                                    || phase.contains(&TurnPhase::Main2),
                            )
                        })
                        .unwrap_or(false);
                    let within_phase = phase_restrictions
                        .as_ref()
                        .and_then(|(phase, trigger_target)| {
                            Some(
                                phase.contains(&turn_phase)
                                    && match trigger_target {
                                        TriggerTarget::Owner => is_owner,
                                        TriggerTarget::Opponent => !is_owner,
                                        TriggerTarget::Any => true,
                                    },
                            )
                        })
                        .unwrap_or(true);

                    let can_pay_mana = if let Some(owner) = card.owner.as_ref() {
                        owner.lock().await.can_pay_mana(required_mana).await
                    } else {
                        false
                    };
                    let player_id = if let Some(owner) = card.owner.as_ref() {
                        Some(owner.lock().await.name.clone())
                    } else {
                        None
                    };
                    let meets_mana_requirements = if let Some(owner) = card.owner.as_ref() {
                        owner.lock().await.has_required_mana(required_mana).await
                    } else {
                        false
                    };
                    // if can_pay_mana && within_phase {
                    let mut meets_requirements_except_mana = within_phase
                        && in_play
                        && ((!card.tapped && card.current_phase == CardPhase::Ready)
                            || !required_tap);
                    if let Some(game_arc) = &game_arc {
                        if let Some(card) = &original_card_arc {
                            meets_requirements_except_mana = meets_requirements_except_mana
                                && (&trigger.requirements)(
                                    Arc::clone(game_arc),
                                    Arc::clone(card),
                                    trigger.id.clone(),
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
                            ActionType::Instant
                        },
                        show: meets_requirements_except_mana
                            || phase_restrictions.is_none()
                            || main_phase_restriction,
                        meets_requirements_except_mana,
                        meets_mana_requirements,
                        can_pay_mana,
                        owner_player_id: player_id,
                    });
                    // return (trigger.card_required_target.clone(), action_type);
                    // }
                }
                x => {}
            }
        }

        if !added_play_from_hand_action && !in_play {
            let mut meets_mana_requirements = false;
            let mut can_pay_mana = false;
            let mut player_id = None;

            if let Some(owner) = &card.owner {
                meets_mana_requirements = owner.lock().await.has_required_mana(&card.cost).await;
                can_pay_mana = owner.lock().await.can_pay_mana(&card.cost).await;
                player_id = Some(owner.lock().await.name.clone());
            }
            abilities.push(AbilityDetails {
                id: "play_card".to_string(),
                action_type: ActionType::PlayedCard,
                mana_cost: vec![],
                required_target: CardRequiredTarget::None,
                description: "Play".to_string(),
                show: false,
                meets_requirements_except_mana: vec![TurnPhase::Main, TurnPhase::Main2]
                    .contains(&turn_phase),
                meets_mana_requirements,
                can_pay_mana,
                owner_player_id: player_id,
            });
        }

        abilities
    }

    pub async fn from_card(card: Card) -> CardWithDetails {
        let frontend_target = FrontendCardTarget {
            player_id: "".to_string(),
            pile: FrontendPileName::Deck,
            card_index: 0,
        };
        let in_play = frontend_target.pile == FrontendPileName::Play;
        let abilities =
            CardWithDetails::get_abilities(&card, TurnPhase::Upkeep, true, None, None).await;
        CardWithDetails {
            card,
            abilities,
            frontend_target,
        }
    }

    pub async fn from_card_arc(
        card_arc: Arc<Mutex<Card>>,
        game: &Arc<Mutex<Game>>,
    ) -> CardWithDetails {
        let card = card_arc.lock().await.clone();
        let turn_phase = game.lock().await.current_phase();
        let frontend_target = game.lock().await.frontend_target_from_card(&card_arc).await;
        let in_play = frontend_target.pile == FrontendPileName::Play;
        let abilities = CardWithDetails::get_abilities(
            &card,
            turn_phase,
            in_play,
            Some(card_arc.clone()),
            Some(Arc::clone(game)),
        )
        .await;
        CardWithDetails {
            card,
            abilities,
            frontend_target,
        }
    }

    // pub async fn from_card(
    //     card: Card,
    //     turn_phase: TurnPhase,
    //     in_play: bool,
    //     frontend_target: FrontendCardTarget,
    // ) -> CardWithDetails {
    //     let abilities =
    //         CardWithDetails::get_abilities(&card, turn_phase, in_play, None, None).await;
    //     CardWithDetails {
    //         card,
    //         abilities,
    //         frontend_target,
    //     }
    // }
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct PriorityQueue {
    pub player_id: String,
    pub time_left: i16,
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
    pub sub: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub player: Arc<Mutex<Player>>,
}
impl PlayerState {
    pub(crate) fn from_player(
        player: Arc<Mutex<Player>>,
        player_index: i32,
        sub: String,
    ) -> PlayerState {
        PlayerState {
            public_info: PublicPlayerInfo {
                cards_in_play: vec![],
                spells: vec![],
                hand_size: 0,
                mana_pool: ManaPool::new(),
                health: 10,
            },
            deck: DeckSelector::Elves,
            sub,
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

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum FrontendPileName {
    Hand,
    Play,
    Spell,
    Deck,
    Exiled,
    Graveyard,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub enum FrontendTarget {
    Card(FrontendCardTarget),
    Player(i32),
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct FrontendCardTarget {
    pub player_id: String,
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
    pub health: i16,
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
    pub effect_manager: EffectManager,
    #[serde(skip_serializing, skip_deserializing)]
    pub event_stack: Vec<Arc<dyn Action + Send + Sync>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub combat: Combat,
    #[serde(skip_serializing, skip_deserializing)]
    pub current_priority_player: Option<(Arc<Mutex<Player>>, i16, ActionType)>,
    #[serde(skip_serializing, skip_deserializing)]
    pub broadcast_sender: Option<broadcast::Sender<Option<LobbyCommand>>>,
    pub turn_messages: Vec<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub choose_action: Option<ChooseFromSelectionAction>,
    #[serde(skip_serializing, skip_deserializing)]
    pub async_abilities: HashMap<String, Ability>,
    #[serde(skip_serializing, skip_deserializing)]
    pub cards_triggered_this_turn: HashSet<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub triggers_played_this_turn: HashMap<String, i32>,
    pub starting_health: i16,
}

impl fmt::Debug for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Game")
            .field("players", &self.players)
            .field("current_turn", &self.current_turn)
            .field("turn_number", &self.turn_number)
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
    canceled:
        Option<Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>>,
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
        canceled: Option<
            Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
        >,
        description: String,
        action_type: ActionType,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            card_arc,
            mana_cost,
            target,
            ability,
            canceled,
            action_type,
            description,
        }
    }
}

impl Game {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);

        Self {
            triggers_played_this_turn: HashMap::new(),
            players: vec![],
            current_turn: None,
            turn_number: 0,
            effect_manager: EffectManager::new(),
            event_stack: vec![],
            combat: Combat::new(),
            current_priority_player: None,
            broadcast_sender: Some(sender),
            turn_messages: vec![],
            async_abilities: HashMap::new(),
            cards_triggered_this_turn: HashSet::new(),
            choose_action: None,
            starting_health: 20,
        }
    }

    pub async fn add_health(&mut self, player: &Arc<Mutex<Player>>, amount: i16) {
        player.lock().await.add_health(amount).await;
        let mut actions = self.collect_health_gained_actions(player).await;
        self.execute_actions(&mut actions).await.ok();
    }

    pub async fn ask_choose_from_selection(&mut self, action: ChooseFromSelectionAction) {
        if let Some(ref sender) = self.broadcast_sender {
            sender.send(Some(LobbyCommand::ChooseFromSelection(action.details)));
        }
    }

    pub async fn ask_mandatory_player_ability(game: &Arc<Mutex<Game>>, ability: Ability) {
        game.lock()
            .await
            .async_abilities
            .insert(ability.id.clone(), ability.clone());
        let sender = game.lock().await.broadcast_sender.clone();
        if let Some(ref sender) = sender {
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
                    CardWithDetails::from_card_arc(ability.card_arc.clone(), game).await,
                    ability.action_type,
                    ability.mana_cost,
                    ability.target,
                    ability.description,
                    ability.id,
                    true,
                    true,
                    true,
                ),
            )));
        }
    }

    pub async fn request_player_ability(game: &Arc<Mutex<Game>>, ability: Ability) {
        game.lock()
            .await
            .async_abilities
            .insert(ability.id.clone(), ability.clone());
        let sender = game.lock().await.broadcast_sender.clone();
        if let Some(ref sender) = sender {
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
                CardWithDetails::from_card_arc(ability.card_arc.clone(), game).await,
                ability.action_type,
                ability.mana_cost,
                ability.target,
                ability.description,
                ability.id,
                true,
                true,
                true,
            ))));
        }
    }

    pub async fn filter_cards_in_play(
        &self,
        closure: Arc<
            dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync,
        >,
    ) -> Vec<Arc<Mutex<Card>>> {
        let mut cards = vec![];
        for player in &self.players {
            let cards_in_play = { player.lock().await.cards_in_play.clone() };
            for card_arc in cards_in_play {
                if (closure)(card_arc.clone()).await {
                    cards.push(card_arc.clone());
                }
            }
        }

        cards
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

    pub async fn respond_card_selection(
        game_arc: Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        if let Some(EffectTarget::Card(card)) = &target {
            let action = { game_arc.lock().await.choose_action.clone() };
            if let Some(ability) = action {
                let action = (ability.action)(Arc::clone(card));
                game_arc
                    .lock()
                    .await
                    .add_to_stack(Arc::new(CardActionWrapper {
                        card: Arc::clone(card),
                        action,
                        target: target.clone(),
                        ability_id: None,
                    }));
                game_arc.lock().await.resolve_stack().await?;
            }
        }

        Ok(())
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
            game.async_abilities
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
                            player.lock().await.pay_mana(&ability.mana_cost).await.ok();
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
        } else {
            let mut game = game_arc.lock().await;
            game.cancel_ability(ability_id).await?;
        }

        Ok(())
    }

    pub async fn cancel_ability(&mut self, ability_id: String) -> Result<(), String> {
        let ability = self
            .async_abilities
            .remove(&ability_id)
            .ok_or("No ability with that id".to_string())?;

        if let Some(canceled) = ability.canceled {
            let card_arc = ability.card_arc.clone();
            let action = (canceled)(card_arc.clone());
            self.add_to_stack(Arc::new(CardActionWrapper {
                card: card_arc.clone(),
                action,
                target: Some(EffectTarget::Card(card_arc)),
                ability_id: Some(ability_id),
            }));
            self.resolve_stack().await?;
        }

        Ok(())
    }

    pub async fn execute_ability(
        &mut self,
        ability_id: String,
        target: Option<EffectTarget>,
    ) -> Result<(), String> {
        let ability = self
            .async_abilities
            .remove(&ability_id)
            .ok_or("No ability with that id".to_string())?;

        let card_arc = ability.card_arc.clone();
        let action = (ability.ability)(card_arc.clone());
        self.add_to_stack(Arc::new(CardActionWrapper {
            card: card_arc,
            action,
            target,
            ability_id: Some(ability_id),
        }));
        self.resolve_stack().await?;

        Ok(())
    }

    pub async fn get_player_from_frontend_target(
        game: &Arc<Mutex<Game>>,
        target: &FrontendCardTarget,
    ) -> Result<(usize, Arc<Mutex<Player>>), String> {
        let game = game.lock().await;
        for (index, player) in game.players.iter().enumerate() {
            if player.lock().await.name == target.player_id {
                return Ok((index, Arc::clone(player)));
            }
        }

        Err("Unable to find target".to_string())
    }

    pub async fn player_from_frontend_target(
        &self,
        target: &FrontendCardTarget,
    ) -> Result<(usize, Arc<Mutex<Player>>), String> {
        for (index, player) in self.players.iter().enumerate() {
            if player.lock().await.name == target.player_id {
                return Ok((index, Arc::clone(player)));
            }
        }

        Err("Unable to find target".to_string())
    }

    pub async fn remove_from_frontend_target(
        &self,
        target: &FrontendCardTarget,
    ) -> Arc<Mutex<Card>> {
        let (target_index, player) = self
            .player_from_frontend_target(&target)
            .await
            .expect("Unable to get player from frontend target");

        match target.pile {
            FrontendPileName::Deck => {
                let player = Arc::clone(&self.players[target_index]);
                let card = player
                    .lock()
                    .await
                    .deck
                    .draw_pile
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Hand => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player
                    .lock()
                    .await
                    .cards_in_hand
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Play => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player
                    .lock()
                    .await
                    .cards_in_play
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Spell => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player
                    .lock()
                    .await
                    .spells
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Exiled => {
                let player = Arc::clone(&self.players[target_index]);
                let card = player
                    .lock()
                    .await
                    .deck
                    .exiled
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Graveyard => {
                let player = Arc::clone(&self.players[target_index]);
                let card = player
                    .lock()
                    .await
                    .deck
                    .graveyard
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
        }
    }

    pub async fn card_from_frontend_target(&self, target: &FrontendCardTarget) -> Arc<Mutex<Card>> {
        let mut target_index = 0;
        for (index, player) in self.players.iter().enumerate() {
            if player.lock().await.name == target.player_id {
                target_index = index;
                break;
            }
        }
        match target.pile {
            FrontendPileName::Deck => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.deck.draw_pile[target.card_index as usize];
                Arc::clone(&card)
            }
            FrontendPileName::Hand => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.cards_in_hand[target.card_index as usize];
                Arc::clone(&card)
            }
            FrontendPileName::Play => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.cards_in_play[target.card_index as usize];
                Arc::clone(&card)
            }
            FrontendPileName::Spell => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.spells[target.card_index as usize];
                Arc::clone(&card)
            }
            FrontendPileName::Exiled => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.deck.exiled[target.card_index as usize];
                Arc::clone(&card)
            }
            FrontendPileName::Graveyard => {
                let player = Arc::clone(&self.players[target_index]);
                let card = &player.lock().await.deck.graveyard[target.card_index as usize];
                Arc::clone(&card)
            }
        }
    }

    pub async fn frontend_target_from_card(&self, arc: &Arc<Mutex<Card>>) -> FrontendCardTarget {
        for (player_index, player) in self.players.iter().enumerate() {
            let player_id = player.lock().await.name.clone();
            for (card_index, card) in player.clone().lock().await.cards_in_hand.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Hand,
                        player_id: player_id,
                    };
                }
            }
            for (card_index, card) in player.clone().lock().await.spells.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Spell,
                        player_id: player_id,
                    };
                }
            }
            for (card_index, card) in player.clone().lock().await.cards_in_play.iter().enumerate() {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Play,
                        player_id: player_id,
                    };
                }
            }
            for (card_index, card) in player
                .clone()
                .lock()
                .await
                .deck
                .draw_pile
                .iter()
                .enumerate()
            {
                if Arc::ptr_eq(card, &arc) {
                    return FrontendCardTarget {
                        card_index: card_index as i32,
                        pile: FrontendPileName::Deck,
                        player_id: player_id.clone(),
                    };
                }
            }
        }

        FrontendCardTarget {
            card_index: 0,
            pile: FrontendPileName::Hand,
            player_id: "".to_string(),
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
            EffectTarget::CardId(_) => todo!(),
        }
    }

    pub fn add_to_stack(&mut self, action: Arc<dyn Action + Send + Sync>) {
        self.event_stack.push(action);
    }

    pub async fn reset_creature_damage(&mut self) {
        for player_arc in &self.players {
            let player = player_arc.lock().await;
            for card_arc in &player.cards_in_play {
                let mut card = card_arc.lock().await;
                if card.card_type == CardType::Creature {
                    card.damage_taken = 0;
                    card.damage_dealt_to_players = 0;
                }
            }
        }
    }

    pub async fn resolve_stack(&mut self) -> Result<(), String> {
        while let Some(action) = self.event_stack.pop() {
            println!("Applying action {:?}", action);
            action.apply(self).await?;
        }

        for player_arc in &self.players {
            let mut player = player_arc.lock().await;
            player.reset_spells();
        }

        if let Some(current_turn) = &self.current_turn.clone() {
            self.effect_manager
                .apply_effects(current_turn.clone())
                .await;
        } else {
            println!("uhhhhhhh? {:?}", self);
        }
        println!("Resolved stack!");
        self.refresh_clients();
        Ok(())
    }

    pub async fn destroy_dead_creatures(&mut self) {
        let mut cards_to_destroy = vec![];
        {
            for player_arc in self.players.clone() {
                let player = player_arc.lock().await;
                for card_arc in &player.cards_in_play {
                    let card = card_arc.lock().await;
                    if card.card_type == CardType::Creature
                        && card.get_stat_value(StatType::Toughness) <= 0
                    {
                        cards_to_destroy.push(Arc::clone(card_arc));
                    }
                }
            }
        }
        for card_arc in cards_to_destroy {
            self.destroy_card(&card_arc).await;
        }
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

        self.execute_actions(&mut actions).await.ok();
    }

    pub async fn detach_card(&mut self, card_arc: &Arc<Mutex<Card>>) {
        let mut card = card_arc.lock().await;
        if let Some(attached_card) = card.attached.take() {
            self.effect_manager
                .remove_effects_by_source(card_arc, self.current_turn.clone().unwrap())
                .await;
        }
    }

    pub async fn resolve_combat(&mut self) {
        let mut touched_cards = HashMap::new();
        for (attacker, blocker) in self.combat.blockers.clone() {
            touched_cards.insert(blocker.clone().lock().await.id.clone(), blocker);
            touched_cards.insert(attacker.clone().lock().await.id.clone(), attacker);
        }
        for (attacker, _) in self.combat.attackers.clone() {
            touched_cards.insert(attacker.clone().lock().await.id.clone(), attacker);
        }
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let destroyed_cards = self.combat.resolve_combat().await;
        for (player_index, player) in self.players.iter().enumerate() {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (card_index, card_in_play) in cards_in_play.iter().enumerate() {
                let triggers = card_in_play.lock().await.triggers.clone();
                if touched_cards.contains_key(card_in_play.lock().await.id.as_str()) {
                    // let target = Some(EffectTarget::Card(Arc::clone(card)));
                    let current_card_owner = { card_in_play.lock().await.owner.clone() };
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(LifeLinkAction {}),
                        target: None,
                        card: Arc::clone(card_in_play),
                        ability_id: None,
                    }));
                    if let Some(current_card_owner) = &current_card_owner {
                        for trigger in triggers {
                            if ActionTriggerType::DamageApplied == trigger.trigger_type {
                                actions.push(Arc::new(CardActionWrapper {
                                    action: trigger.action.clone(),
                                    card: Arc::clone(card_in_play),
                                    target: None,
                                    ability_id: Some(trigger.id.clone()),
                                }));
                            }
                        }
                    }
                }
            }
        }

        self.execute_actions(&mut actions).await.ok();
        for card in destroyed_cards {
            self.destroy_card(&card).await;
        }
        self.handle_deaths().await;
    }

    pub async fn destroy_card(&mut self, card: &Arc<Mutex<Card>>) {
        self.remove_references_to(card).await;
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let owner = card.lock().await.owner.clone();
        if let Some(card_owner) = &owner {
            for (player_index, player) in self.players.iter().enumerate() {
                let cards_in_play = player.lock().await.cards_in_play.clone();
                for (card_index, card_in_play) in cards_in_play.iter().enumerate() {
                    if Arc::ptr_eq(card, card_in_play) {
                        {
                            player.lock().await.destroy_card_in_play(card_index).await
                        };
                    }

                    let triggers = card_in_play.lock().await.triggers.clone();
                    let target = Some(EffectTarget::Card(Arc::clone(card)));
                    let current_card_owner = { card.lock().await.owner.clone() };
                    if let Some(current_card_owner) = &current_card_owner {
                        for trigger in triggers {
                            if ActionTriggerType::CardDestroyed == trigger.trigger_type
                                && Arc::ptr_eq(card, card_in_play)
                            {
                                actions.push(Arc::new(CardActionWrapper {
                                    action: trigger.action.clone(),
                                    card: Arc::clone(card),
                                    target: None,
                                    ability_id: Some(trigger.id.clone()),
                                }));
                            }
                            if let ActionTriggerType::OtherCardDestroyed(trigger_target) =
                                &trigger.trigger_type
                            {
                                match trigger_target {
                                    TriggerTarget::Opponent => todo!(),
                                    _ => {
                                        if Arc::ptr_eq(card, card_in_play) {
                                            continue;
                                        }
                                        if !Arc::ptr_eq(card_owner, current_card_owner) {
                                            continue;
                                        }
                                        actions.push(Arc::new(CardActionWrapper {
                                            action: trigger.action.clone(),
                                            card: Arc::clone(card),
                                            target: target.clone(),
                                            ability_id: Some(trigger.id.clone()),
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            println!("destroy actions? {:?}", actions);
            self.execute_actions(&mut actions).await.ok();
        }
    }

    pub async fn add_player(&mut self, player: Player) -> Arc<Mutex<Player>> {
        let player_arc = Arc::new(Mutex::new(player));
        player_arc.lock().await.deck.set_owner(&player_arc).await;
        self.players.push(Arc::clone(&player_arc));

        player_arc
    }

    // TODO: remove this in favor of activate card action
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

        self.execute_actions(&mut actions).await?;
        self.destroy_dead_creatures().await;

        Ok(())
    }

    pub async fn activate_card_action(
        game: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        card: FrontendCardTarget,
        target: Option<EffectTarget>,
        trigger_id: String,
    ) -> Result<(), String> {
        if let Some((current_player, _, action_taken)) =
            &mut game.lock().await.current_priority_player
        {
            if !Arc::ptr_eq(&player, current_player) {
                return Err("Not your turn".to_string());
            } else {
                *action_taken = ActionType::Tap;
            }
        }

        if trigger_id == "play_card".to_string() {
            println!("play card triggered!!!!!");
            Game::play_card(game, &card, target).await?;
            return Ok(());
        }

        let response = Player::execute_action(
            Arc::clone(player),
            card.card_index as usize,
            target,
            Arc::clone(game),
            trigger_id,
        )
        .await;

        match response {
            Ok(mut actions) => {
                let mut game = game.lock().await;
                game.execute_actions(&mut actions).await?;
                game.destroy_dead_creatures().await;
                Ok(())
            }
            Err(err) => Err(err),
        }
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
        game_arc: &Arc<Mutex<Game>>,
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

        let result = Game::execute_card_from_hand(game_arc, player_arc, index, None).await?;
        game_arc.lock().await.resolve_stack().await?;

        // Now pass the game Arc to process the action queue
        // Game::process_action_queue(game_arc.clone(), result.clone()).await;

        Ok(result)
    }

    pub async fn exiled_card_to_battlefield(
        game: &Arc<Mutex<Game>>,
        card_id: String,
    ) -> Result<(), String> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let players = game.lock().await.players.clone();
        for (player_index, player) in players.iter().enumerate() {
            let exiled = player.lock().await.deck.exiled.clone();
            for (index, exiled_card) in exiled.iter().enumerate() {
                let id = exiled_card.lock().await.id.clone();
                if id == card_id {
                    Game::play_card_without_mana(
                        game,
                        &FrontendCardTarget {
                            player_id: player.lock().await.name.clone(),
                            pile: FrontendPileName::Exiled,
                            card_index: index as i32,
                        },
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        Err("Hmm, unable to find that card".to_string())
    }

    pub async fn exile_card(&mut self, card: &Arc<Mutex<Card>>) -> Result<(), String> {
        self.remove_references_to(card).await;
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let owner = card.lock().await.owner.clone();
        if let Some(card_owner) = &owner {
            for (player_index, player) in self.players.iter().enumerate() {
                let cards_in_play = player.lock().await.cards_in_play.clone();
                for (card_index, card_in_play) in cards_in_play.iter().enumerate() {
                    if Arc::ptr_eq(card, card_in_play) {
                        {
                            println!("exiling card!");
                            player.lock().await.exile_card_in_play(card_index).await;
                        };
                    }

                    let triggers = card_in_play.lock().await.triggers.clone();
                    let target = Some(EffectTarget::Card(Arc::clone(card)));
                    let current_card_owner = { card.lock().await.owner.clone() };
                    if let Some(current_card_owner) = &current_card_owner {
                        for trigger in triggers {
                            if ActionTriggerType::CardExiled == trigger.trigger_type
                                && Arc::ptr_eq(card, card_in_play)
                            {
                                actions.push(Arc::new(CardActionWrapper {
                                    action: trigger.action.clone(),
                                    card: Arc::clone(card),
                                    target: None,
                                    ability_id: Some(trigger.id.clone()),
                                }));
                            }
                            if let ActionTriggerType::OtherCardExiled(trigger_target) =
                                &trigger.trigger_type
                            {
                                match trigger_target {
                                    TriggerTarget::Opponent => todo!(),
                                    _ => {
                                        if Arc::ptr_eq(card, card_in_play) {
                                            continue;
                                        }
                                        if !Arc::ptr_eq(card_owner, current_card_owner) {
                                            continue;
                                        }
                                        actions.push(Arc::new(CardActionWrapper {
                                            action: trigger.action.clone(),
                                            card: Arc::clone(card),
                                            target: target.clone(),
                                            ability_id: Some(trigger.id.clone()),
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            println!("exiled actions? {:?}", actions);
            self.execute_actions(&mut actions).await?;
            self.add_turn_message(format!("{} was exiled.", card.lock().await.name));
        }

        Ok(())
    }

    pub async fn play_card(
        game_arc: &Arc<Mutex<Game>>,
        card: &FrontendCardTarget,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let (_, player) = game_arc
            .lock()
            .await
            .player_from_frontend_target(card)
            .await?;

        {
            if let Some((current_player, _, action_taken)) =
                &mut game_arc.lock().await.current_priority_player
            {
                if !Arc::ptr_eq(&player, current_player) {
                    return Err("Not your turn".to_string());
                } else {
                    *action_taken = ActionType::PlayedCard;
                }
            }
        }

        let card = Game::execute_play_card(game_arc, card, target).await?;
        Ok(card)
    }

    pub async fn play_card_from_hand(
        game_arc: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        {
            if let Some((current_player, _, action_taken)) =
                &mut game_arc.lock().await.current_priority_player
            {
                if !Arc::ptr_eq(&player, current_player) {
                    return Err("Not your turn".to_string());
                } else {
                    *action_taken = ActionType::PlayedCard;
                }
            }
        }

        let card = Game::execute_card_from_hand(game_arc, player, index, target).await?;
        Ok(card)
    }

    async fn get_card_from_frontend_position(
        game_arc: &Arc<Mutex<Game>>,
        position: &FrontendCardTarget,
    ) -> Arc<Mutex<Card>> {
        game_arc
            .lock()
            .await
            .card_from_frontend_target(position)
            .await
    }

    async fn play_card_without_mana(
        game_arc: &Arc<Mutex<Game>>,
        card: &FrontendCardTarget,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let card = {
            let game = game_arc.lock().await;
            game.remove_from_frontend_target(card).await
        };
        game_arc.lock().await.resolve_stack().await?;
        println!("removed card {}", card.lock().await.name);
        let game = Arc::clone(game_arc);
        let player = card.lock().await.owner.clone().unwrap();
        // let card = player.lock().await.deck.draw_pile[index].clone();
        let card_cloned = card.clone();
        println!("playing card {}", card_cloned.lock().await.name);
        let action = Arc::new(PlayCardAction::new(player, card_cloned.clone(), None));
        {
            let mut game = game.lock().await;

            game.add_to_stack(action);
            game.resolve_stack().await?;
            game.destroy_dead_creatures().await;
        }

        Ok(card)
    }

    async fn execute_play_card(
        game_arc: &Arc<Mutex<Game>>,
        frontend_card: &FrontendCardTarget,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let card = game_arc
            .lock()
            .await
            .remove_from_frontend_target(frontend_card)
            .await;
        let (_, player) = Game::get_player_from_frontend_target(game_arc, frontend_card).await?;
        Player::play_card(
            &player,
            &card,
            game_arc.lock().await.current_turn.clone().unwrap(),
        )
        .await?;

        let game = Arc::clone(game_arc);
        let card_cloned = card.clone();
        tokio::spawn(async move {
            let is_spell = { card_cloned.lock().await.card_type.is_spell().clone() };
            if is_spell {
                Game::priority_loop(game.clone(), card_cloned.clone()).await;
            }
            let action = Arc::new(PlayCardAction::new(player, card_cloned.clone(), target));
            {
                let mut game = game.lock().await;

                game.add_to_stack(action);
                game.resolve_stack().await.ok();
                game.destroy_dead_creatures().await;
            }
        });

        Ok(card)
    }

    async fn execute_card_from_hand(
        game_arc: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        index: usize,
        target: Option<EffectTarget>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let card = {
            Player::play_card_in_hand(
                player,
                index,
                game_arc.lock().await.current_turn.clone().unwrap(),
            )
            .await?
        };

        let game = Arc::clone(game_arc);
        let player = Arc::clone(player);
        let card_cloned = card.clone();
        tokio::spawn(async move {
            let is_spell = { card_cloned.lock().await.card_type.is_spell().clone() };
            if is_spell {
                Game::priority_loop(game.clone(), card_cloned.clone()).await;
            }
            let action = Arc::new(PlayCardAction::new(player, card_cloned.clone(), None));
            {
                let mut game = game.lock().await;

                game.add_to_stack(action);
                game.resolve_stack().await.ok();
                game.destroy_dead_creatures().await;
            }
        });

        Ok(card)
    }
    pub fn refresh_clients(&self) {
        if let Some(ref sender) = self.broadcast_sender {
            let _ = sender.send(None);
        }
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

                for card_in_play in &cards {
                    let triggers = card_in_play.lock().await.triggers.clone();
                    let target = Some(EffectTarget::Card(Arc::clone(card_arc)));
                    for trigger in triggers {
                        if let ActionTriggerType::CardPlayedFromHand(_) = &trigger.trigger_type {
                            if let Some(_) = trigger.action.as_any().downcast_ref::<BlankAction>() {
                                continue;
                            }
                            if Arc::ptr_eq(card_in_play, card_arc) {
                                actions.push(Arc::new(CardActionWrapper {
                                    action: trigger.action.clone(),
                                    card: Arc::clone(card_in_play),
                                    target: target.clone(),
                                    ability_id: Some(trigger.id.clone()),
                                }));
                            }
                        }

                        if let ActionTriggerType::CreatureTypeCardPlayed(
                            trigger_target,
                            creature_type,
                        ) = &trigger.trigger_type
                        {
                            match trigger_target {
                                TriggerTarget::Opponent => todo!(),
                                TriggerTarget::Owner => {
                                    let current_card_owner =
                                        { card_in_play.lock().await.owner.clone() };
                                    if let Some(current_card_owner) = &current_card_owner {
                                        if !Arc::ptr_eq(owner, current_card_owner) {
                                            continue;
                                        }
                                        let card_creature_type =
                                            { card_in_play.lock().await.creature_type };
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
                                                    card: Arc::clone(card_in_play),
                                                    target: target.clone(),
                                                    ability_id: Some(trigger.id.clone()),
                                                }));
                                                // here
                                            }
                                        }
                                    }
                                }
                                TriggerTarget::Any => todo!(),
                            }
                        }
                        if Arc::ptr_eq(card_in_play, card_arc) {
                            continue;
                        }
                        if let ActionTriggerType::OtherCardPlayed(trigger_target) =
                            &trigger.trigger_type
                        {
                            let current_card_owner = { card_in_play.lock().await.owner.clone() };
                            if let Some(current_card_owner) = &current_card_owner {
                                match trigger_target {
                                    TriggerTarget::Opponent => todo!(),
                                    TriggerTarget::Owner => {
                                        if !Arc::ptr_eq(owner, current_card_owner) {
                                            continue;
                                        }
                                        println!(
                                            "{} other card played triggered for {}",
                                            card_in_play.lock().await.name,
                                            card_arc.lock().await.name
                                        );

                                        actions.push(Arc::new(CardActionWrapper {
                                            action: trigger.action.clone(),
                                            card: Arc::clone(card_in_play),
                                            target: target.clone(),
                                            ability_id: Some(trigger.id.clone()),
                                        }));
                                    }
                                    TriggerTarget::Any => todo!(),
                                }
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
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(DestroyTargetCAction {}),
                        card: Arc::clone(card_rc),
                        target: None,
                        ability_id: None,
                    }));
                }
            }
        }

        actions
    }

    pub async fn collect_card_stat_changed_actions(
        &mut self,
        card: &Arc<Mutex<Card>>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        let triggers = card.lock().await.triggers.clone();
        let target = Some(EffectTarget::Card(Arc::clone(&card)));
        for trigger in triggers {
            match trigger.trigger_type {
                ActionTriggerType::CardStatChanged => actions.push(Arc::new(CardActionWrapper {
                    action: trigger.action,
                    card: Arc::clone(card),
                    target: target.clone(),
                    ability_id: Some(trigger.id.clone()),
                })),
                _ => {}
            }
        }

        actions
    }

    pub async fn collect_health_gained_actions(
        &mut self,
        player: &Arc<Mutex<Player>>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        let cards = player.lock().await.cards_in_play.clone();

        for card in &cards {
            let triggers = card.lock().await.triggers.clone();
            let target = Some(EffectTarget::Card(Arc::clone(&card)));
            for trigger in triggers {
                match trigger.trigger_type {
                    ActionTriggerType::HealthGained => actions.push(Arc::new(CardActionWrapper {
                        action: trigger.action,
                        card: Arc::clone(card),
                        target: target.clone(),
                        ability_id: Some(trigger.id.clone()),
                    })),
                    _ => {}
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
                                ability_id: Some(trigger.id.clone()),
                            }))
                        }
                        _ => {}
                    }
                }
            }
        }

        actions
    }

    pub async fn execute_actions(
        &mut self,
        actions: &mut Vec<Arc<dyn Action + Send + Sync>>,
    ) -> Result<(), String> {
        let actions_to_execute = std::mem::take(actions);

        for action in actions_to_execute {
            self.event_stack.push(action);
        }

        self.resolve_stack().await?;

        let actions_to_execute = self.collect_omnipresent_actions().await;
        for action in actions_to_execute {
            self.event_stack.push(action);
        }

        self.resolve_stack().await
    }

    pub fn reset_turn_messages(&mut self) {
        self.turn_messages = vec![];
        self.messages_updated();
    }

    pub async fn start_turn(&mut self, player_index: usize) {
        self.reset_turn_messages();

        let player_id = self.players[player_index].lock().await.name.clone();

        self.current_turn = Some(Turn::new(
            self.players[player_index].clone(),
            player_index,
            player_id,
            self.turn_number,
        ));
        self.turn_number += 1;

        self.reset_creature_damage().await;

        self.triggers_played_this_turn.clear();
        self.cards_triggered_this_turn.clear();

        for player in &self.players {
            let mut player = player.lock().await;
            player.health_at_start_of_round = player.stat_manager.get_stat_value(StatType::Health);
        }

        let player_arc = self.players[player_index].clone();
        {
            let mut player = player_arc.lock().await;
            player.advance_card_phases().await;
        }
        println!("started turn?");
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
        action.apply(self).await?;

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
        initial_time_limit: i16,
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
                        //         *tl = time_left as i16;
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
                    *tl = time_left as i16;
                }
            }

            time_since_last_notification += sleep_duration;

            if time_since_last_notification >= Duration::from_secs(1) {
                time_since_last_notification = Duration::from_secs(0);

                {
                    let game = game_arc.lock().await;
                    game.refresh_clients();
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
                    let player = player_arc.lock().await;
                    player.priority_turn_start().await;
                    println!("Player {}'s priority turn has started.", player.name);
                }
                let time_limit = if i == 0 { 0 } else { 3 };

                {
                    let mut game = game_arc.lock().await;
                    game.current_priority_player =
                        Some((player_arc.clone(), time_limit.clone(), ActionType::None));

                    game.refresh_clients();
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

                player_arc.lock().await.priority_turn_end().await;
            }

            if passed_players.iter().all(|&passed| passed) {
                println!("All players have passed. Exiting priority loop.");

                game_arc.lock().await.refresh_clients();

                break;
            }
        }

        game_arc.lock().await.current_priority_player = None;
    }

    // pub async fn process_action_queue(game_arc: Arc<Mutex<Game>>, card_arc: Arc<Mutex<Card>>) {
    //     let card = { card_arc.lock().await.clone() };

    //     if card.card_type.is_spell() {
    //         {
    //             let mut game = game_arc.lock().await;
    //             game.add_turn_message(format!(
    //                 "{} is casting {}",
    //                 card.owner.unwrap().lock().await.name,
    //                 card.name
    //             ));
    //         }

    //         Self::priority_loop(Arc::clone(&game_arc), card_arc).await;
    //     }

    //     let mut game = game_arc.lock().await;

    //     game.resolve_stack().await;

    //     if let Some(ref sender) = game.broadcast_sender {
    //         let _ = sender.send(None);
    //     }
    // }

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
            self.execute_actions(&mut actions).await.ok();

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

    pub async fn start(game: &Arc<Mutex<Game>>) {
        let players = game.lock().await.players.clone();
        for player_arc in &players {
            let mut player = player_arc.lock().await;
            // player
            //     .stat_manager
            //     .add_listener(Arc::new(Box::new(GameStatChangeListener {
            //         card: Arc::clone(card_arc),
            //         game: Arc::clone(game),
            //     })
            //         as Box<dyn CardStatChangeListener + Send + Sync>));

            player.deck.first_shuffle().await;
            for _ in 0..7 {
                player.draw_card();
            }
        }
        game.lock().await.start_turn(0).await;
    }
}
