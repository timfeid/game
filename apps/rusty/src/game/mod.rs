use std::{
    borrow::{Borrow, BorrowMut},
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    thread::{current, Thread},
    time::Duration,
};

use action::{
    Action, ActionTriggerType, AsyncClosureAction, BlankAction, CardAction, CardActionTarget,
    CardActionTrigger, CardActionWrapper, CardRequiredTarget, ChooseFromSelectionAction,
    ChosenCardDetails, CombatDamageAction, DestroySelf, DestroyTargetCAction, LifeLinkAction,
    PhaseTarget, PlayCardAction, PlayerActionWrapper,
};
use card::{Card, CardPhase, CardType};
use combat::Combat;
use effects::{Effect, EffectID, EffectManager, EffectTarget};
use futures::{channel::oneshot, stream, StreamExt};
use mana::{ManaPool, ManaType};
use player::{CardPosition, Player};
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
        ModalButton,
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
    None,
    ActivatedAbility,
    ManaAbility,
    Spell,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone)]
pub struct CardWithDetails {
    pub position: Option<CardPosition>,
    pub card: Card,
    pub frontend_target: FrontendCardTarget,
    pub abilities: Vec<AbilityDetails>,
    pub attached_to: Option<FrontendCardTarget>,
    pub stats: HashMap<StatType, i16>,
    pub incremental_counters: HashMap<String, i16>,
}

impl CardWithDetails {
    pub async fn from_card(card: Card) -> CardWithDetails {
        let frontend_target = FrontendCardTarget {
            player_id: "".to_string(),
            pile: FrontendPileName::Deck,
            card_index: 0,
        };

        let abilities = card.abilities(TurnPhase::Main, true, None).await;
        let attached_to = None;
        let stats = HashMap::new();
        let incremental_counters = HashMap::new();

        CardWithDetails {
            position: None,
            card,
            abilities,
            frontend_target,
            attached_to,
            stats,
            incremental_counters,
        }
    }

    pub async fn from_card_arc(
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
    ) -> CardWithDetails {
        let card = card_arc.lock().await.clone();
        let phase = game.clone().lock().await.current_phase();
        let frontend_target = Game::frontend_target_from_card(&game, &card_arc)
            .await
            .expect("hmm ?");
        let attached_to = if let Some(attached) = &card.attached {
            Game::frontend_target_from_card(&game, attached)
                .await
                .ok()
                .and_then(|x| Some(x.1))
        } else {
            None
        };
        let abilities = card
            .abilities(
                phase,
                frontend_target.1.pile == FrontendPileName::Play,
                Some((card_arc.clone(), game)),
            )
            .await;
        let stats = card_arc.lock().await.stats.to_hashmap();
        let incremental_counters = card_arc.lock().await.get_incremental_counters();

        CardWithDetails {
            card,
            abilities,
            frontend_target: frontend_target.1,
            attached_to,
            position: frontend_target.0,
            stats,
            incremental_counters,
        }
    }
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
                health: 20,
                luck_tokens: 0,
            },
            deck: DeckSelector::Vegas,
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

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub enum FrontendTarget {
    Card(FrontendCardTarget),
    Player(String),
    FrontlineBattlefield,
    BacklineBattlefield,
}

#[derive(Type, Deserialize, Serialize, Debug, Clone, PartialEq)]
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
    pub luck_tokens: i16,
}

pub enum PriorityActionResult {
    NoAction,              // Player did nothing
    ActionRequiresRestart, // Player performed an action that requires restarting the priority loop
    Timeout,               // Player did not act in time
    ExtendedTime,
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
    player_arc: Arc<Mutex<Player>>,
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
        player_arc: Arc<Mutex<Player>>,
        mana_cost: Vec<ManaType>,
        target: CardRequiredTarget,
        ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
        canceled: Option<
            Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
        >,
        description: String,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            card_arc,
            mana_cost,
            target,
            ability,
            canceled,
            description,
            player_arc,
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

    pub async fn ask_choose_from_selection(&mut self, action: ChooseFromSelectionAction) {
        self.choose_action = Some(action.clone());
        if let Some(ref sender) = self.broadcast_sender {
            sender
                .send(Some(LobbyCommand::ChooseFromSelection(action.details)))
                .ok();
        }
    }

    pub async fn ask_mandatory_player_ability(&mut self, ability: Ability) {
        self.async_abilities
            .insert(ability.id.clone(), ability.clone());
        let sender = self.broadcast_sender.clone();
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
                    CardWithDetails::from_card(ability.card_arc.lock().await.clone()).await,
                    vec![],
                    ability.target,
                    ability.description,
                    ability.id,
                    true,
                    true,
                    true,
                    None,
                ),
            )));
        }
    }

    pub async fn request_player_ability(&mut self, ability: Ability) {
        self.async_abilities
            .insert(ability.id.clone(), ability.clone());
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
                CardWithDetails::from_card(ability.card_arc.lock().await.clone()).await,
                ability.mana_cost,
                ability.target,
                ability.description,
                ability.id,
                true,
                true,
                true,
                None,
            ))));
        }
    }

    pub async fn filter_cards_in_play<F>(&self, closure: F) -> Vec<Arc<Mutex<Card>>>
    where
        F: Fn(&Card) -> bool + 'static + Send + Sync,
    {
        let mut cards = vec![];
        for player in &self.players {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (_, card_arc) in cards_in_play.iter() {
                // Attempt to acquire the lock with a 50ms timeout
                match timeout(Duration::from_millis(50), card_arc.lock()).await {
                    Ok(card_guard) => {
                        let card = card_guard; // Successfully acquired the lock
                        if (closure)(&card) {
                            cards.push(card_arc.clone());
                        }
                    }
                    Err(_) => {
                        // Timeout occurred, skip this card
                        continue;
                    }
                }
            }
        }

        cards
    }

    pub async fn has_tapped_creature_excluding(&self, cards: &Vec<Arc<Mutex<Card>>>) -> bool {
        for player in &self.players {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (index, (_, card_arc)) in cards_in_play.iter().enumerate() {
                let card = card_arc.lock().await;
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

        false
    }

    pub async fn respond_card_selection_button(
        game: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        button_id: String,
    ) -> Result<(), String> {
        let choose_action = game.lock().await.choose_action.clone();
        if let Some(ability) = choose_action {
            if let Some(action) = ability.details.buttons.iter().find(|b| b.id == button_id) {
                if let Some(action) = &action.action {
                    (action)().await?;
                }
            }
        }
        game.lock().await.choose_action = None;

        Ok(())
    }

    pub async fn respond_card_selection(
        game: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        selected_card: Option<FrontendTarget>,
    ) -> Result<(), String> {
        if let Some(FrontendTarget::Card(card)) = &selected_card {
            let card_arc = Game::card_from_frontend_card_target(game, card).await;
            let choose_action = game.lock().await.choose_action.clone();
            if let Some(ability) = choose_action {
                if let Some(action) = ability.action {
                    (action)(ChosenCardDetails::new(card_arc, card.clone())).await?;
                }
            }
        }
        game.lock().await.choose_action = None;

        Ok(())
    }

    pub async fn respond_player_ability(
        game: &Arc<Mutex<Game>>,
        player: &Arc<Mutex<Player>>,
        ability_id: String,
        response: bool,
        target: Option<FrontendTarget>,
    ) -> Result<(), String> {
        let ability = game
            .lock()
            .await
            .async_abilities
            .get(&ability_id)
            .ok_or_else(|| "No ability with that id".to_string())?
            .clone();

        let phase = game.lock().await.current_phase();
        let player = Arc::clone(player);

        if response {
            println!("Processing response for ability: {:?}", ability);
            let game_arc = Arc::clone(game);
            if !ability.mana_cost.is_empty() {
                println!("it has a cost!");
                let cloned_ability_id = ability_id.clone();
                let game = Arc::clone(game);
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
                        let can_pay_mana =
                            player.lock().await.has_required_mana(&ability.mana_cost);

                        if can_pay_mana {
                            println!("can pay mana");
                            player.lock().await.pay_mana(&ability.mana_cost).ok();
                            Game::execute_ability(&game, cloned_ability_id, target)
                                .await
                                .ok();
                            return;
                        }

                        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    }
                });
            } else {
                Game::execute_ability(&game, ability_id, target).await?;
            }
        } else {
            Game::cancel_ability(&game, ability_id).await?;
        }

        Ok(())
    }

    pub async fn cancel_ability(game: &Arc<Mutex<Game>>, ability_id: String) -> Result<(), String> {
        let ability = game
            .lock()
            .await
            .async_abilities
            .remove(&ability_id)
            .ok_or("No ability with that id".to_string())?;

        if let Some(canceled) = ability.canceled {
            let card_arc = ability.card_arc.clone();
            let action = (canceled)(card_arc.clone());
            Game::execute_actions(
                Arc::clone(game),
                vec![Arc::new(CardActionWrapper {
                    card: card_arc,
                    action,
                    target: None,
                    ability_id: Some(ability_id),
                })],
            )
            .await?;
        }

        Ok(())
    }

    pub async fn execute_ability(
        game: &Arc<Mutex<Game>>,
        ability_id: String,
        target: Option<FrontendTarget>,
    ) -> Result<(), String> {
        let ability = game
            .lock()
            .await
            .async_abilities
            .remove(&ability_id)
            .ok_or("No ability with that id".to_string())?;

        let card_arc = ability.card_arc.clone();
        let action = (ability.ability)(card_arc.clone());
        Game::execute_actions(
            Arc::clone(game),
            vec![Arc::new(CardActionWrapper {
                card: card_arc,
                action,
                target,
                ability_id: Some(ability_id),
            })],
        )
        .await?;

        Ok(())
    }

    pub async fn get_player_from_frontend_target(
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

    pub async fn player_from_frontend_target(
        game: &Arc<Mutex<Game>>,
        target: &FrontendTarget,
    ) -> Result<(usize, Arc<Mutex<Player>>), String> {
        if let FrontendTarget::Player(player) = target {
            let players = game.lock().await.players.clone();
            for (index, current_player) in players.iter().enumerate() {
                let name = current_player.lock().await.name.clone();
                if name == *player {
                    return Ok((index, Arc::clone(current_player)));
                }
            }
        }

        Err("Unable to find target".to_string())
    }

    pub async fn player_from_id(
        game: &Arc<Mutex<Game>>,
        id: &str,
    ) -> Result<(usize, Arc<Mutex<Player>>), String> {
        let players = game.lock().await.players.clone();
        for (index, player) in players.iter().enumerate() {
            if player.lock().await.name.clone() == id {
                return Ok((index, Arc::clone(player)));
            }
        }

        Err("Unable to find target".to_string())
    }

    pub async fn player_from_frontend_card_target(
        &self,
        target: &FrontendCardTarget,
    ) -> Result<(usize, Arc<Mutex<Player>>), String> {
        for (index, player) in self.players.iter().enumerate() {
            if player.lock().await.name.clone() == target.player_id {
                return Ok((index, Arc::clone(player)));
            }
        }

        Err("Unable to find target".to_string())
    }

    pub async fn remove_card_from_current_pile(game: &Arc<Mutex<Game>>, card: &Arc<Mutex<Card>>) {
        let target = Game::frontend_target_from_card(game, card).await;
        if let Ok((_, target)) = &target {
            game.lock().await.remove_from_frontend_target(target).await;
        }
    }

    pub async fn remove_from_frontend_target(
        &self,
        target: &FrontendCardTarget,
    ) -> Arc<Mutex<Card>> {
        let (target_index, player) = self
            .player_from_frontend_card_target(&target)
            .await
            .expect("Unable to get player from frontend target");

        match target.pile {
            FrontendPileName::Deck => {
                let card = player
                    .lock()
                    .await
                    .deck
                    .draw_pile
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Hand => {
                let card = &player
                    .lock()
                    .await
                    .cards_in_hand
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Play => {
                let (_, card) = &player
                    .lock()
                    .await
                    .cards_in_play
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Spell => {
                let card = &player
                    .lock()
                    .await
                    .spells
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Exiled => {
                let card = player
                    .lock()
                    .await
                    .deck
                    .exiled
                    .remove(target.card_index as usize);
                Arc::clone(&card)
            }
            FrontendPileName::Graveyard => {
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

    pub async fn card_to_effect_target(
        game: &Arc<Mutex<Game>>,
        card: &Arc<Mutex<Card>>,
    ) -> EffectTarget {
        let (_, target) = Game::frontend_target_from_card(game, card)
            .await
            .expect("not found");
        Game::frontend_card_to_effect_target(game, &FrontendTarget::Card(target)).await
    }

    pub async fn frontend_card_to_effect_target(
        game: &Arc<Mutex<Game>>,
        target: &FrontendTarget,
    ) -> EffectTarget {
        match target {
            FrontendTarget::Card(frontend_card_target) => EffectTarget::Card(
                Game::card_from_frontend_card_target(game, frontend_card_target).await,
            ),
            FrontendTarget::Player(_) => EffectTarget::Player(
                Game::player_from_frontend_target(game, target)
                    .await
                    .expect("Unable to find player")
                    .1,
            ),
            _ => panic!("unable to target that"),
        }
    }

    pub async fn card_from_frontend_target(
        game: &Arc<Mutex<Game>>,
        target: &FrontendTarget,
    ) -> Arc<Mutex<Card>> {
        if let FrontendTarget::Card(card) = target {
            return Game::card_from_frontend_card_target(game, card).await;
        }
        panic!("No card found on the frontend like that");
    }

    pub async fn card_in_play(game: &Arc<Mutex<Game>>, target: &FrontendCardTarget) -> bool {
        target.pile == FrontendPileName::Play
    }

    pub async fn card_from_frontend_card_target(
        game: &Arc<Mutex<Game>>,
        target: &FrontendCardTarget,
    ) -> Arc<Mutex<Card>> {
        let mut player = None;
        for (index, current_player) in { game.lock().await.players.clone() }.iter().enumerate() {
            if current_player.lock().await.name == target.player_id {
                player = Some(Arc::clone(current_player));
                break;
            }
        }
        let player = player.expect("No player found");
        let player = player.lock().await;
        match target.pile {
            FrontendPileName::Deck => {
                let card = player.deck.draw_pile[target.card_index as usize].clone();
                Arc::clone(&card)
            }
            FrontendPileName::Hand => {
                let card = player.cards_in_hand[target.card_index as usize].clone();
                Arc::clone(&card)
            }
            FrontendPileName::Play => {
                let (_, card) = player.cards_in_play[target.card_index as usize].clone();
                Arc::clone(&card)
            }
            FrontendPileName::Spell => {
                let card = player.spells[target.card_index as usize].clone();
                Arc::clone(&card)
            }
            FrontendPileName::Exiled => {
                let card = player.deck.exiled[target.card_index as usize].clone();
                Arc::clone(&card)
            }
            FrontendPileName::Graveyard => {
                let card = player.deck.graveyard[target.card_index as usize].clone();
                Arc::clone(&card)
            }
        }
    }

    pub async fn frontend_target_from_card(
        game: &Arc<Mutex<Game>>,
        target: &Arc<Mutex<Card>>,
    ) -> Result<(Option<CardPosition>, FrontendCardTarget), String> {
        let target_id = target.lock().await.id.clone();
        let mut card_position = None;

        let players = game.lock().await.players.clone();
        for (player_index, player) in players.iter().enumerate() {
            let player_id = player.lock().await.name.clone();
            for (card_index, card_in_play) in { player.lock().await.cards_in_hand.clone() }
                .iter()
                .enumerate()
            {
                if card_in_play.lock().await.id == target_id {
                    return Ok((
                        card_position,
                        FrontendCardTarget {
                            card_index: card_index as i32,
                            pile: FrontendPileName::Hand,
                            player_id: player_id,
                        },
                    ));
                }
            }
            for (card_index, card) in { player.lock().await.spells.clone() }.iter().enumerate() {
                if card.lock().await.id == target_id {
                    return Ok((
                        card_position,
                        FrontendCardTarget {
                            card_index: card_index as i32,
                            pile: FrontendPileName::Spell,
                            player_id: player_id,
                        },
                    ));
                }
            }
            for (card_index, (position, card)) in { player.lock().await.cards_in_play.clone() }
                .iter()
                .enumerate()
            {
                card_position = Some(position.clone());
                if card.lock().await.id == target_id {
                    return Ok((
                        card_position,
                        FrontendCardTarget {
                            card_index: card_index as i32,
                            pile: FrontendPileName::Play,
                            player_id: player_id,
                        },
                    ));
                }
            }
            for (card_index, card) in { player.lock().await.deck.graveyard.clone() }
                .iter()
                .enumerate()
            {
                if card.lock().await.id == target_id {
                    return Ok((
                        card_position,
                        FrontendCardTarget {
                            card_index: card_index as i32,
                            pile: FrontendPileName::Graveyard,
                            player_id: player_id.clone(),
                        },
                    ));
                }
            }
            for (card_index, card) in { player.lock().await.deck.draw_pile.clone() }
                .iter()
                .enumerate()
            {
                if card.lock().await.id == target_id {
                    return Ok((
                        card_position,
                        FrontendCardTarget {
                            card_index: card_index as i32,
                            pile: FrontendPileName::Deck,
                            player_id: player_id.clone(),
                        },
                    ));
                }
            }
        }

        Err(format!(
            "Unable to find frontend target for card {}",
            target.lock().await.name
        ))
    }

    // pub async fn frontend_target_from_effect_target(
    //     &self,
    //     target: &FrontendTarget,
    // ) -> FrontendTarget {
    //     match target {
    //         FrontendTarget::Player(arc) => FrontendTarget::Player(arc.lock().await.name.clone()),
    //         FrontendTarget::Card(arc) => {
    //             FrontendTarget::Card(self.frontend_target_from_card(arc).await)
    //         }
    //     }
    // }

    pub fn add_to_stack(&mut self, action: Arc<dyn Action + Send + Sync>) {
        self.event_stack.push(action);
    }

    pub async fn reset_creature_damage(&mut self) {
        for player_arc in &self.players {
            let player = player_arc.lock().await;
            for (_, card_arc) in &player.cards_in_play {
                let mut card = card_arc.lock().await;
                if card.card_type == CardType::Creature {
                    card.damage_taken = 0;
                    card.damage_dealt_to_players = 0;
                }
            }
        }
    }

    pub async fn resolve_stack(
        game: &Arc<Mutex<Game>>,
        notify_listeners: bool,
    ) -> Result<(), String> {
        let mut stack = game.lock().await.event_stack.clone();
        stack.reverse();
        Game::execute_actions(Arc::clone(game), stack).await?;
        game.lock().await.event_stack = vec![];

        let players = game.lock().await.players.clone();
        for player_arc in players {
            let mut player = player_arc.lock().await;
            player.reset_spells();
        }

        let turn = game.lock().await.current_turn.clone();
        if let Some(current_turn) = turn {
            game.lock()
                .await
                .effect_manager
                .apply_effects(current_turn)
                .await;
        }

        if notify_listeners {
            game.lock().await.refresh_clients();
        }
        Ok(())
    }

    pub async fn destroy_dead_creatures(game: &Arc<Mutex<Game>>) {
        let mut cards_to_destroy = vec![];
        {
            let players = game.lock().await.players.clone();
            for player_arc in players.iter() {
                let player = player_arc.lock().await;
                for (_, card_arc) in &player.cards_in_play {
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
            Game::destroy_card(game, &card_arc).await;
        }
    }

    pub async fn remove_references_to(&mut self, card: &Arc<Mutex<Card>>) {
        let mut cards_to_detach = vec![];

        for player_arc in self.players.iter() {
            let cards_in_play = player_arc.lock().await.cards_in_play.clone();

            for (_, card_in_play_arc) in cards_in_play {
                let should_detach = {
                    let card_in_play = card_in_play_arc.lock().await;

                    let is_attached = if let Some(attached_arc) = &card_in_play.attached {
                        Arc::ptr_eq(attached_arc, card)
                    } else {
                        false
                    };

                    let is_same_card = Arc::ptr_eq(&card_in_play_arc, card);

                    is_attached || is_same_card
                };

                if should_detach {
                    cards_to_detach.push(card_in_play_arc);
                }
            }
        }

        for card in cards_to_detach {
            self.detach_card(&card).await;
        }
    }

    pub async fn detach_card(&mut self, card_arc: &Arc<Mutex<Card>>) {
        if let Some(_attached_card) = card_arc.lock().await.attached.take() {
            self.effect_manager
                .remove_effects_by_source(card_arc, self.current_turn.clone().unwrap())
                .await;
        }
    }

    async fn card_attacked(
        game: &Arc<Mutex<Game>>,
        attacking_card: Arc<Mutex<Card>>,
        target: FrontendTarget,
    ) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let player = attacking_card.lock().await.owner.clone().unwrap();
        // let cards = {
        //     let mut player = player.lock().await;
        //     player.cards_in_play.clone()
        // };

        // for card in &cards {
        //     if Arc::ptr_eq(card, attacking_card) {
        //         continue;
        //     }

        let triggers = attacking_card.lock().await.triggers.clone();
        // let target = Some(FrontendTarget::Card(Arc::clone(&card)));
        for trigger in triggers {
            match trigger.trigger_type {
                ActionTriggerType::CardAttacked => actions.push(Arc::new(CardActionWrapper {
                    action: trigger.action,
                    card: attacking_card.clone(),
                    target: Some(target.clone()),
                    ability_id: Some(trigger.id.clone()),
                })),
                _ => {}
            }
        }
        // }

        Game::execute_actions(Arc::clone(game), actions).await.ok();
    }

    pub async fn resolve_combat(game: Arc<Mutex<Game>>) {
        let mut touched_cards = HashMap::new();
        let blockers = game.lock().await.combat.blockers.clone();
        for (attacker, blocker) in blockers {
            touched_cards.insert(blocker.clone().lock().await.id.clone(), blocker);
            touched_cards.insert(attacker.clone().lock().await.id.clone(), attacker);
        }
        let attackers = game.lock().await.combat.attackers.clone();
        for (attacker, target) in attackers {
            touched_cards.insert(attacker.clone().lock().await.id.clone(), attacker.clone());
            Game::card_attacked(&game, attacker, target).await;
        }

        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let attackers = game.lock().await.combat.attackers.clone();
        let attackers = Combat::convert_attackers(&game, attackers).await;
        let (destroyed_cards, damages) = game.lock().await.combat.resolve_combat(attackers).await;
        let players = game.lock().await.players.clone();
        for (player_index, player) in players.iter().enumerate() {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (card_index, (_, card_in_play)) in cards_in_play.iter().enumerate() {
                let triggers = card_in_play.lock().await.triggers.clone();
                if touched_cards.contains_key(card_in_play.lock().await.id.as_str()) {
                    // let target = Some(FrontendTarget::Card(Arc::clone(card)));
                    let current_card_owner = { card_in_play.lock().await.owner.clone() };
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(LifeLinkAction {}),
                        target: None,
                        card: Arc::clone(card_in_play),
                        ability_id: None,
                    }));
                }
            }
        }

        for (attacker, target, total) in damages {
            let cloned_game = game.clone();
            Game::execute_action_from_triggers(cloned_game.clone(), move |card, trigger, _| {
                let card = card.clone();
                let trigger = trigger.clone();
                let attacker = attacker.clone();
                let target = target.clone();
                let cloned_game = cloned_game.clone();
                Box::pin(async move {
                    if Arc::ptr_eq(&card, &attacker) {
                        if total > 0 && ActionTriggerType::DamageApplied == trigger.trigger_type {
                            return Some(CardActionWrapper {
                                action: trigger.action.clone(),
                                card: attacker,
                                target: match target {
                                    EffectTarget::Player(arc) => {
                                        Some(FrontendTarget::Player(arc.lock().await.name.clone()))
                                    }
                                    EffectTarget::Card(arc) => Some(FrontendTarget::Card(
                                        Game::frontend_target_from_card(&cloned_game, &arc)
                                            .await
                                            .unwrap()
                                            .1,
                                    )),
                                },
                                ability_id: Some(trigger.id.clone()),
                            });
                        }
                    }
                    None
                })
            })
            .await;
        }

        Game::execute_actions(game.clone(), actions).await.ok();
        for card in destroyed_cards {
            Game::destroy_card(&game, &card).await.ok();
        }
    }

    pub async fn destroy_card(
        game: &Arc<Mutex<Game>>,
        card: &Arc<Mutex<Card>>,
    ) -> Result<(), String> {
        game.lock().await.remove_references_to(card).await;
        Game::remove_card_from_current_pile(game, card).await;

        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let owner = card.lock().await.owner.clone();
        if let Some(card_owner) = &owner {
            card_owner
                .lock()
                .await
                .deck
                .graveyard
                .push(Arc::clone(card));
            let players = game.lock().await.players.clone();
            for (player_index, player) in players.iter().enumerate() {
                let cards_in_play = player.lock().await.cards_in_play.clone();
                for (card_index, (_, card_in_play)) in cards_in_play.iter().enumerate() {
                    let triggers = card_in_play.lock().await.triggers.clone();
                    // let target = Some(self.get);
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
                                    PhaseTarget::Opponent => todo!(),
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
                                            target: Some(FrontendTarget::Card(
                                                Game::frontend_target_from_card(game, card_in_play)
                                                    .await?
                                                    .1,
                                            )),
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
            Game::execute_actions(Arc::clone(game), actions).await?;
        }
        Ok(())
    }

    pub async fn add_player(&mut self, player: Player) -> Arc<Mutex<Player>> {
        let player_arc = Arc::new(Mutex::new(player));
        player_arc.lock().await.deck.set_owner(&player_arc).await;
        self.players.push(Arc::clone(&player_arc));

        player_arc
    }

    pub async fn activate_card_action(
        game: &Arc<Mutex<Game>>,
        card_target: FrontendCardTarget,
        target: Option<FrontendTarget>,
        trigger_id: String,
    ) -> Result<(), String> {
        let (player_index, player) = game
            .lock()
            .await
            .player_from_frontend_card_target(&card_target)
            .await?;

        let in_play = card_target.pile == FrontendPileName::Play;
        let card = Game::card_from_frontend_card_target(game, &card_target).await;

        // Modify collect_manual_actions to return an ActionType
        let (mut actions, tap, mana, action_type) = Card::collect_manual_actions(
            card.clone(),
            in_play,
            target.clone(),
            trigger_id.clone(),
            game,
        )
        .await?;

        {
            // Check if it's the player's priority turn
            let mut game_guard = game.lock().await;
            if let Some((current_player, _, action_taken)) = &mut game_guard.current_priority_player
            {
                if !Arc::ptr_eq(&player, current_player) {
                    return Err("Not your turn".to_string());
                } else {
                    *action_taken = action_type.clone();
                }
            }
        }

        // Player pays mana cost
        player.lock().await.pay_mana(&mana)?;

        // Tap the card if required
        if tap {
            card.lock().await.tap()?;
        }

        match action_type {
            ActionType::ManaAbility => {
                // Mana abilities are executed immediately
                println!("mana!");
                Game::execute_actions(game.clone(), actions).await?;
            }
            ActionType::ActivatedAbility | ActionType::Spell => {
                if action_type == ActionType::Spell {
                    let triggers = card.lock().await.triggers.clone();
                    for trigger in triggers {
                        if let ActionTriggerType::Instant = &trigger.trigger_type {
                            Game::execute_actions(
                                game.clone(),
                                vec![Arc::new(CardActionWrapper {
                                    action: trigger.action.clone(),
                                    card: card.clone(),
                                    target: target.clone(),
                                    ability_id: Some(trigger.id.clone()),
                                })],
                            )
                            .await?;
                        }
                    }

                    if let Ok((_, card_target)) =
                        Game::frontend_target_from_card(&game, &card).await
                    {
                        game.lock()
                            .await
                            .remove_from_frontend_target(&card_target)
                            .await;
                    }
                    player.lock().await.spells.push(card.clone());
                }
                // Add actions to the event stack
                println!("added {:?} to the stack for {}", actions, trigger_id);
                game.lock().await.event_stack.append(&mut actions);

                // Start the priority loop if it's not already running
                // if game.lock().await.current_priority_player.is_none() {
                //     let game_cloned = game.clone();
                //     let player_cloned = player.clone();
                //     tokio::spawn(async move {
                //         Game::priority_loop(game_cloned.clone(), &player_cloned).await;
                Game::resolve_stack(game, true).await.ok();
                //     });
                // }
            }
            _ => {
                // For other action types, execute immediately
                Game::execute_actions(game.clone(), actions).await?;
            }
        }

        Ok(())
    }

    pub async fn play_token(
        game_arc: &Arc<Mutex<Game>>,
        player_arc: &Arc<Mutex<Player>>,
        mut token: Card,
    ) -> Result<Arc<Mutex<Card>>, String> {
        token.owner = Some(Arc::clone(player_arc));
        // let card = Arc::new(Mutex::new(token));

        // let index = {
        //     let mut player = player_arc.lock().await;
        //     player.cards_in_hand.push(card.clone());
        //     player.cards_in_hand.len() - 1
        // };

        // let result = self.execute_card_from_hand(game_arc, player_arc, index, None).await?;

        // Now pass the game Arc to process the action queue
        // self.process_action_queue(game_arc.clone(), result.clone()).await;
        println!("gonna have to re-do this one?");

        todo!()
    }

    pub async fn exiled_card_to_battlefield(game: &Arc<Mutex<Game>>, card_id: String) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        println!("looping players");
        let players = game.lock().await.players.clone();
        for (player_index, player) in players.iter().enumerate() {
            println!("looping exileds");
            let exiled = player.lock().await.deck.exiled.clone();
            for (index, exiled_card) in exiled.iter().enumerate() {
                println!("reading card {:?}", exiled_card);
                let id = exiled_card.lock().await.id.clone();
                if id == card_id {
                    println!("found it!");
                    // execute action ?
                }
            }
        }
    }

    pub async fn exile_card(
        game: &Arc<Mutex<Game>>,
        card: &Arc<Mutex<Card>>,
    ) -> Result<(), String> {
        game.lock().await.remove_references_to(card).await;
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = vec![];
        let owner = card.lock().await.owner.clone();
        if let Some(card_owner) = &owner {
            let players = game.lock().await.players.clone();
            for (player_index, player) in players.iter().enumerate() {
                let cards_in_play = player.lock().await.cards_in_play.clone();
                for (card_index, (_, card_in_play)) in cards_in_play.iter().enumerate() {
                    if Arc::ptr_eq(card, card_in_play) {
                        {
                            println!("exiling card!");
                            player.lock().await.exile_card_in_play(card_index).await;
                        };
                    }

                    let triggers = card_in_play.lock().await.triggers.clone();
                    // let target = Some(FrontendTarget::Card(Arc::clone(card)));
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
                                    PhaseTarget::Opponent => todo!(),
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
                                            target: Some(FrontendTarget::Card(
                                                Game::frontend_target_from_card(game, card)
                                                    .await?
                                                    .1,
                                            )),
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
            Game::execute_actions(Arc::clone(game), actions).await?;
            game.lock()
                .await
                .add_turn_message(format!("{} was exiled.", card.lock().await.name));
        }

        Ok(())
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

    pub fn debug(&self, message: &str) {
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
        target: &Option<FrontendTarget>,
    ) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let owner = card_arc.lock().await.owner.clone();
        if let Some(owner) = &owner {
            for player in &self.players {
                let cards = player.lock().await.cards_in_play.clone();

                for (_, card_in_play) in &cards {
                    let triggers = card_in_play.lock().await.triggers.clone();
                    for trigger in triggers {
                        if let ActionTriggerType::CardEnteredBattlefield = &trigger.trigger_type {
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
                                PhaseTarget::Opponent => todo!(),
                                PhaseTarget::Owner => {
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
                                PhaseTarget::Any => todo!(),
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
                                    PhaseTarget::Opponent => todo!(),
                                    PhaseTarget::Owner => {
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
                                    PhaseTarget::Any => todo!(),
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

        for (player_index, player_arc) in self.players.iter().enumerate() {
            let (triggers, cards_in_play) = {
                let player = player_arc.lock().await;
                (player.triggers.clone(), player.cards_in_play.clone())
            };
            for trigger in &triggers {
                if trigger
                    .applies_in_phase(self.current_turn.as_ref().unwrap(), Arc::clone(player_arc))
                    .await
                {
                    actions.push(Arc::new(PlayerActionWrapper {
                        action: Arc::clone(&trigger.action),
                        player: Arc::clone(player_arc),
                    })
                        as Arc<(dyn Action + std::marker::Send + Sync)>);
                }
            }

            // Collect actions for each card the player has in play
            for (_, card_arc) in &cards_in_play {
                let turn = self.current_turn.clone().unwrap();
                let triggers = card_arc.lock().await.triggers.clone();
                for action_trigger in &triggers {
                    if let ActionTriggerType::PhaseStarted(trigger_phase, phase_target) =
                        &action_trigger.trigger_type
                    {
                        let is_owner = Arc::ptr_eq(&turn.current_player, &player_arc);
                        let target = card_arc.lock().await.target.clone();
                        if trigger_phase.contains(&turn.phase)
                            && match phase_target {
                                PhaseTarget::Owner => is_owner,
                                PhaseTarget::Opponent => !is_owner,
                                PhaseTarget::Any => true,
                            }
                        {
                            actions.push(Arc::new(CardActionWrapper {
                                card: card_arc.clone(),
                                action: action_trigger.action.clone(),
                                target,
                                ability_id: Some(action_trigger.id.clone()),
                            }));
                        }
                    }
                }
                let has_effects = self.effect_manager.has_effects(&card_arc).await;
                if card_arc.lock().await.is_useless(has_effects) {
                    println!("{} is useless", card_arc.lock().await.name);
                    actions.push(Arc::new(CardActionWrapper {
                        action: Arc::new(DestroySelf {}),
                        card: card_arc.clone(),
                        target: None,
                        ability_id: None,
                    }));
                }
            }
        }

        actions
    }

    pub async fn collect_omnipresent_actions(&self) -> Vec<Arc<dyn Action + Send + Sync>> {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();

        for player in &self.players {
            let cards = player.lock().await.cards_in_play.clone();

            for (_, card) in &cards {
                let triggers = card.lock().await.triggers.clone();
                let target = card.lock().await.target.clone();
                for trigger in triggers {
                    if let ActionTriggerType::Omnipresent = trigger.trigger_type {
                        actions.push(Arc::new(CardActionWrapper {
                            action: trigger.action,
                            card: Arc::clone(card),
                            target: target.clone(),
                            ability_id: Some(trigger.id.clone()),
                        }))
                    }
                }
            }
        }

        actions
    }

    pub async fn execute_actions(
        game: Arc<Mutex<Self>>, // Pass the game as Arc<Mutex<Game>>
        actions: Vec<Arc<dyn Action + Send + Sync>>,
    ) -> Result<(), String> {
        for action in actions {
            action.apply(game.clone()).await?;
        }
        // Lock the game to collect omnipresent actions
        let actions_to_execute = {
            let game_locked = game.lock().await;
            game_locked.collect_omnipresent_actions().await
        };

        // Lock the game to push more actions onto the stack
        {
            for action in actions_to_execute {
                action.apply(game.clone()).await?;
            }
        }

        {
            let mut game_locked = game.lock().await;
            game_locked.handle_deaths().await;
        }

        game.lock().await.refresh_clients();

        Ok(())
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

        let players = self.players.clone();
        for player_arc in players {
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
        game: Arc<Mutex<Game>>,
        player_arc: Arc<Mutex<Player>>,
        action: Arc<dyn Action + Send + Sync>,
    ) -> Result<(), String> {
        action.apply(game).await?;

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

    pub async fn priority_loop(game_arc: Arc<Mutex<Game>>, first_player: &Arc<Mutex<Player>>) {
        let mut players_in_order = {
            let game = game_arc.lock().await;
            game.debug("Entering priority loop");
            game.get_players_in_priority_order(first_player)
        };

        loop {
            let num_players = players_in_order.len();
            let mut passed_players = vec![false; num_players];

            for (i, player_arc) in players_in_order.iter().enumerate() {
                if passed_players[i] {
                    continue;
                }

                {
                    let player = player_arc.lock().await;
                    player.priority_turn_start().await;
                    println!("Player {}'s priority turn has started.", player.name);
                }

                let initial_time_limit = if i == 0 { 0 } else { 3 };

                {
                    let mut game = game_arc.lock().await;
                    game.current_priority_player =
                        Some((player_arc.clone(), initial_time_limit, ActionType::None));
                    game.refresh_clients();
                }

                let result =
                    Game::wait_for_player_action_async(game_arc.clone(), initial_time_limit).await;

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
                    PriorityActionResult::ExtendedTime => {
                        println!(
                            "{} performed an action, extending their time.",
                            player_arc.lock().await.name
                        );
                        // Extend the player's time by looping again without marking them as passed
                        continue;
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

    fn reset_performed_action(&mut self) {
        if let Some((_, _, action)) = &mut self.current_priority_player {
            *action = ActionType::None;
        }
    }

    pub async fn wait_for_player_action_async(
        game_arc: Arc<Mutex<Game>>,
        initial_time_limit: i16,
    ) -> PriorityActionResult {
        let sleep_duration = Duration::from_millis(100);
        let mut deadline = Instant::now() + Duration::from_secs(initial_time_limit as u64);
        let mut time_since_last_notification = Duration::from_secs(0);

        loop {
            if Instant::now() >= deadline {
                return PriorityActionResult::Timeout;
            }

            {
                let mut game = game_arc.lock().await;
                match game.performed_action() {
                    ActionType::Spell => {
                        println!("Player played a card, restarting priority loop.");
                        // Reset the action performed flag
                        game.reset_performed_action();
                        return PriorityActionResult::ActionRequiresRestart;
                    }
                    ActionType::None => {
                        // No action performed
                    }
                    action => {
                        println!("Player performed action {:?}, extending timer.", action);
                        // Reset the action performed flag
                        game.reset_performed_action();
                        // Extend the deadline
                        deadline = Instant::now() + Duration::from_secs(15);

                        // Update the current_priority_player's time limit
                        if let Some((_, ref mut time_limit, _)) = game.current_priority_player {
                            *time_limit = 15;
                        }

                        // Return ExtendedTime to inform the priority_loop to continue without passing
                        return PriorityActionResult::ExtendedTime;
                    }
                }
            }

            let time_left = deadline.saturating_duration_since(Instant::now()).as_secs();
            {
                let mut game = game_arc.lock().await;
                if let Some((_, ref mut time_limit, _)) = game.current_priority_player {
                    *time_limit = time_left as i16;
                }
            }

            time_since_last_notification += sleep_duration;

            if time_since_last_notification >= Duration::from_secs(1) {
                time_since_last_notification = Duration::from_secs(0);
                let game = game_arc.lock().await;
                game.refresh_clients();
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

    pub async fn apply_effect<E>(game: &Arc<Mutex<Game>>, effect: E)
    where
        E: 'static + Effect + Send + Sync,
    {
        game.lock()
            .await
            .effect_manager
            .add_effect(effect.get_final_id(), Arc::new(Mutex::new(effect)));
        let turn = game.lock().await.current_turn.clone();
        if let Some(current_turn) = turn {
            game.lock()
                .await
                .effect_manager
                .apply_effects(current_turn)
                .await;
        }

        game.lock().await.refresh_clients();
    }

    pub fn messages_updated(&self) {
        if let Some(ref sender) = self.broadcast_sender {
            let _ = sender.send(Some(LobbyCommand::TurnMessages(LobbyTurnMessage {
                messages: self.turn_messages.clone(),
            })));
        }
    }

    pub async fn advance_turn(game: &Arc<Mutex<Game>>) {
        if let Some((current_player, _, _)) = game.lock().await.current_priority_player.clone() {
            println!("Cannot advance turn while waiting for priority queue.");
            return;
        }

        loop {
            // Clone the current turn and update state inside a scoped lock
            let mut changed_player = None;
            {
                let mut game_locked = game.lock().await;
                let len = game_locked.players.len();

                if let Some(ref mut turn) = game_locked.current_turn {
                    // Advance to the next phase
                    turn.next_phase();

                    if turn.phase == TurnPhase::Untap {
                        let next_player_index = (turn.current_player_index + 1) % len as i32;

                        // Start the new player's turn
                        changed_player = Some(next_player_index);
                        // Increment turn number if we've completed a round
                        if next_player_index == 0 {
                            turn.turn_number += 1;
                        }
                    }
                }
            }; // Lock is released here

            if let Some(player) = changed_player {
                game.lock().await.start_turn(player as usize).await;
            }

            let actions = game.lock().await.collect_actions_for_phase().await;

            // Now call execute_actions without holding the game lock
            Game::execute_actions(Arc::clone(game), actions).await.ok();

            // Log the current phase
            {
                let game_locked = game.lock().await;
                println!(
                    ":: TURN ADVANCED :: {:?} effects: {:?}",
                    game_locked.current_turn.clone().unwrap().phase,
                    ""
                );
            }

            // Check the current phase and determine if we should advance further
            let mut should_advance = {
                let game_locked = game.lock().await;
                let current_phase = game_locked.current_phase();

                match current_phase {
                    TurnPhase::Main => false,
                    TurnPhase::DeclareAttackers => {
                        game_locked
                            .current_turn
                            .as_ref()
                            .unwrap()
                            .current_player
                            .lock()
                            .await
                            .filter_cards_in_play(|card| card.card_type == CardType::Creature)
                            .await
                            .len()
                            == 0
                    }
                    TurnPhase::DeclareBlockers => game_locked.combat.attackers.len() == 0,
                    _ => true,
                }
            };

            if game.lock().await.current_phase() == TurnPhase::DeclareBlockers && !should_advance {
                Game::blockers_turn(game).await;
                should_advance = true;
            }

            // Break the loop if we shouldn't advance any further
            if !should_advance {
                break;
            }
        }

        game.lock().await.refresh_clients();
    }

    async fn blockers_turn(game: &Arc<Mutex<Game>>) {
        let owner = game
            .lock()
            .await
            .current_turn
            .as_ref()
            .unwrap()
            .current_player
            .clone();
        Game::priority_loop(Arc::clone(game), &owner).await;
        Game::resolve_stack(game, false).await.ok();
        Game::priority_loop(Arc::clone(game), &owner).await;
        Game::resolve_stack(game, true).await.ok();
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
        Game::advance_turn(game).await;
    }

    pub async fn execute_action_from_triggers<F>(game: Arc<Mutex<Game>>, closure: F)
    where
        F: Fn(
                Arc<Mutex<Card>>,
                CardActionTrigger,
                Arc<Mutex<Player>>,
            ) -> Pin<Box<dyn Future<Output = Option<CardActionWrapper>> + Send>>
            + 'static
            + Send
            + Sync,
    {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let players = game.lock().await.players.clone();
        for player in &players {
            let cards_in_play = player.lock().await.cards_in_play.clone();
            for (_, card_arc) in cards_in_play.iter() {
                // Attempt to acquire the lock with a 50ms timeout
                match timeout(Duration::from_millis(50), card_arc.lock()).await {
                    Ok(card_guard) => {
                        let card = card_guard; // Successfully acquired the lock
                        for trigger in &card.triggers {
                            if let Some(action) = (closure)(
                                card_arc.clone(),
                                trigger.clone(),
                                card.owner.clone().unwrap(),
                            )
                            .await
                            {
                                actions.push(Arc::new(action));
                            };
                        }
                    }
                    Err(_) => {
                        // Timeout occurred, skip this card
                        continue;
                    }
                }
            }
        }

        Game::execute_actions(game, actions).await.expect("hmm?");
    }

    async fn add_stat(
        game: &Arc<Mutex<Game>>,
        source: &Arc<Mutex<Card>>,
        player: &Arc<Mutex<Player>>,
        stat_type: StatType,
        amount: i16,
    ) {
        let mut actions: Vec<Arc<dyn Action + Send + Sync>> = Vec::new();
        let cards = {
            let mut player = player.lock().await;
            player.add_stat_type(stat_type.clone(), amount).await;
            player.cards_in_play.clone()
        };

        for (_, card) in &cards {
            if Arc::ptr_eq(source, card) {
                continue;
            }

            let triggers = card.lock().await.triggers.clone();
            // let target = Some(FrontendTarget::Card(Arc::clone(&card)));
            for trigger in triggers {
                match trigger.trigger_type {
                    ActionTriggerType::PlayerStatChanged(looking_for) => {
                        if looking_for == stat_type {
                            actions.push(Arc::new(CardActionWrapper {
                                action: trigger.action,
                                card: Arc::clone(card),
                                target: None,
                                ability_id: Some(trigger.id.clone()),
                            }))
                        }
                    }
                    _ => {}
                }
            }
        }

        Game::execute_actions(Arc::clone(game), actions).await.ok();
    }

    async fn choose_card<F>(
        game: &Arc<Mutex<Game>>,
        player_id: String,
        cards: Vec<Arc<Mutex<Card>>>,
        action: F,
        message: &str,
        selection_required: bool,
    ) where
        F: Fn(ChosenCardDetails) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
            + 'static
            + Send
            + Sync,
    {
        let mut cards_with_detail = vec![];
        for card in cards {
            let with_detail = CardWithDetails::from_card_arc(Arc::clone(game), card).await;
            cards_with_detail.push(with_detail);
        }
        let selection_action = ChooseFromSelectionAction::new(
            player_id,
            cards_with_detail,
            action,
            message,
            selection_required,
        );
        game.lock()
            .await
            .ask_choose_from_selection(selection_action)
            .await;
    }

    async fn show_cards_to_all_except(
        game: &Arc<Mutex<Game>>,
        cards: Vec<Arc<Mutex<Card>>>,
        message: &str,
        player_id: String,
    ) {
        let players = game.lock().await.players.clone();
        for player in &players {
            let current_player_id = player.lock().await.name.clone();
            if player_id == current_player_id {
                continue;
            }
            let mut cards_with_detail = vec![];
            for card in cards.clone() {
                let with_detail = CardWithDetails::from_card_arc(Arc::clone(game), card).await;
                cards_with_detail.push(with_detail);
            }
            let selection_action =
                ChooseFromSelectionAction::show_only(current_player_id, cards_with_detail, message);
            game.lock()
                .await
                .ask_choose_from_selection(selection_action)
                .await;
        }
    }

    async fn show_cards_to_all(
        game: &Arc<Mutex<Game>>,
        cards: Vec<Arc<Mutex<Card>>>,
        message: &str,
    ) {
        let players = game.lock().await.players.clone();
        for player in &players {
            let player_id = player.lock().await.name.clone();
            let mut cards_with_detail = vec![];
            for card in cards.clone() {
                let with_detail = CardWithDetails::from_card_arc(Arc::clone(game), card).await;
                cards_with_detail.push(with_detail);
            }
            let selection_action =
                ChooseFromSelectionAction::show_only(player_id, cards_with_detail, message);
            game.lock()
                .await
                .ask_choose_from_selection(selection_action)
                .await;
        }
    }

    async fn scry(
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
        mut top_cards: Vec<CardWithDetails>,
    ) -> Result<(), String> {
        // Clone the player's name for identification
        let player_id = player.lock().await.name.clone();
        let total = top_cards.len();

        // Shared Sender to communicate between the closure and the main loop
        let sender_shared = Arc::new(Mutex::new(None));
        let selected_cards = Arc::new(Mutex::new(vec![]));

        // Loop until all cards are arranged or the player cancels
        while !top_cards.is_empty() {
            // Create a one-shot channel to wait for the player's selection
            let (tx, rx) = oneshot::channel();

            {
                // Assign the sender to the shared sender
                let mut sender_lock = sender_shared.lock().await;
                *sender_lock = Some(tx);
            }

            // Clone necessary Arcs for the closure
            let game_clone = game.clone();
            let player_clone = player.clone();
            let display_cards = top_cards.clone();
            let sender_shared_clone = sender_shared.clone();
            let selected_cards_clone = selected_cards.clone();
            let message = format!(
                "Look at the top {} cards of your library. You may rearrange them in any order or put them on the bottom of your library.\n\n {}",
                total, if top_cards.len() == total{
                    "Choose one card you would like on top, or put them all at the bottom."
                } else {
                    "Choose the next card or put them all at the bottom."
                }
            );

            // Define a new ChooseFromSelectionAction for the current set of cards
            let mut scry_action = ChooseFromSelectionAction::new(
                player_id.clone(),
                display_cards,
                move |chosen_card| {
                    let sender_shared_clone = sender_shared_clone.clone();
                    let selected_cards_clone = selected_cards_clone.clone();
                    Box::pin(async move {
                        // Access the shared sender and send the chosen card
                        let mut sender_lock = sender_shared_clone.lock().await;
                        if let Some(tx) = sender_lock.take() {
                            // Send Some(chosen_card) to indicate a card was selected
                            let _ = tx.send(Some(chosen_card));
                        }
                        Ok(())
                    })
                },
                message.as_str(),
                true,
            );

            // Add a button to place the rest on the bottom of the deck
            let sender_shared_clone = sender_shared.clone();
            scry_action.details.buttons.push(ModalButton::new(
                "Place the rest on the bottom of the deck",
                move || {
                    let sender_shared_clone = sender_shared_clone.clone();
                    Box::pin(async move {
                        let mut sender_lock = sender_shared_clone.lock().await;
                        if let Some(tx) = sender_lock.take() {
                            // Send None to indicate placing remaining cards on the bottom
                            let _ = tx.send(None);
                        }
                        Ok(())
                    })
                },
            ));

            // Apply the action to the game
            {
                println!("Prompting player for card selection...");
                let mut game_guard = game.lock().await;
                game_guard.choose_action = Some(scry_action.clone());
                game_guard.ask_choose_from_selection(scry_action).await;
            }

            // Wait for the player's selection
            match rx.await {
                Ok(chosen_card_option) => {
                    if let Some(chosen_card) = chosen_card_option {
                        // Add the chosen card to selected_cards
                        selected_cards.lock().await.push(chosen_card.target.clone());
                        // Remove the chosen card from top_cards
                        top_cards.retain(|card| {
                            println!(
                                "\n\n\ncomparing {:?} with {:?}\n\n\n",
                                card.frontend_target, chosen_card.frontend_target
                            );
                            card.frontend_target != chosen_card.frontend_target
                        });
                    } else {
                        // Player chose to place the rest on the bottom
                        break;
                    }
                }
                Err(err) => {
                    // If the player cancels, exit the loop gracefully
                    println!("Player canceled scry: {:?}", err);
                    break;
                }
            }
        }

        // After the loop, arrange the deck accordingly
        {
            // Get the selected cards in the order they were selected
            let selected = selected_cards.lock().await.clone();

            // Reverse the selected cards to have the first selected on top
            let mut selected_reversed = selected.clone();
            selected_reversed.reverse();

            for selected_card in selected_reversed.iter() {
                // Find the card in the deck by `frontend_target`
                Game::remove_card_from_current_pile(&game, selected_card).await;
                player
                    .lock()
                    .await
                    .deck
                    .draw_pile
                    .push(Arc::clone(selected_card));
            }

            // Append the remaining top_cards to the bottom of the deck
            // player.lock().await.deck.draw_pile.extend(top_cards.iter().map(|c| c.frontend_target).clone());
            for card in top_cards {
                let card = Game::card_from_frontend_card_target(&game, &card.frontend_target).await;
                player.lock().await.deck.draw_pile.insert(0, card);
            }
        }

        game.lock().await.refresh_clients();

        println!("Scry completed successfully.");
        Ok(())
    }

    async fn show_cards(
        game: &Arc<Mutex<Game>>,
        player_id: String,
        cards: Vec<Arc<Mutex<Card>>>,
        message: &str,
    ) {
        let mut cards_with_detail = vec![];
        for card in cards {
            let with_detail = CardWithDetails::from_card_arc(Arc::clone(game), card).await;
            cards_with_detail.push(with_detail);
        }
        let selection_action =
            ChooseFromSelectionAction::show_only(player_id, cards_with_detail, message);
        game.lock()
            .await
            .ask_choose_from_selection(selection_action)
            .await;
    }

    async fn draw_card(
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
        source: Option<Arc<Mutex<Card>>>,
    ) -> Result<Arc<Mutex<Card>>, String> {
        let card = player.lock().await.draw_card();
        if let Some(card) = card.clone() {
            let card_cloned = card.clone();
            Game::execute_action_from_triggers(
                game.clone(),
                move |card_with_trigger, trigger, card_owner| {
                    let card = card_cloned.clone();
                    let player = player.clone();
                    let game = game.clone();
                    let source = source.clone();
                    Box::pin(async move {
                        if let ActionTriggerType::OtherCardDrawn(trigger_target) =
                            &trigger.trigger_type
                        {
                            match trigger_target {
                                PhaseTarget::Opponent => todo!(),
                                _ => {
                                    if let Some(source) = &source {
                                        if Arc::ptr_eq(&card, source) {
                                            return None;
                                        }
                                    }
                                    if !Arc::ptr_eq(&player, &card_owner) {
                                        return None;
                                    }

                                    let mut target = None;
                                    if let Some(source) = source {
                                        target = Game::frontend_target_from_card(&game, &source)
                                            .await
                                            .ok()
                                            .and_then(|(_, x)| Some(FrontendTarget::Card(x)));
                                    }

                                    return Some(CardActionWrapper {
                                        action: trigger.action.clone(),
                                        card: card_with_trigger,
                                        target,
                                        ability_id: Some(trigger.id.clone()),
                                    });
                                }
                            }
                        }
                        None
                    })
                },
            )
            .await;
            Ok(card)
        } else {
            Err("No cards left?".into())
        }
    }
}
