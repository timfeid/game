pub mod add_stat;
pub mod generate_mana;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::Mutex;
use ulid::Ulid;
use uuid::Uuid;

use super::{
    card::{Card, CardType, CreatureType},
    effects::{Effect, EffectID, EffectTarget, ExpireContract},
    mana::ManaType,
    player::Player,
    stat::{StatType, Stats},
    turn::{Turn, TurnPhase},
    Ability, ActionType, CardWithDetails, FrontendCardTarget, FrontendTarget, Game,
};
use crate::{
    game::stat::Stat,
    lobby::manager::{CardSelectionDetails, LobbyCommand},
};

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

    pub(crate) async fn applies_in_phase(&self, turn: Turn, player: Arc<Mutex<Player>>) -> bool {
        match &self.trigger_type {
            ActionTriggerType::CardStatChanged => true,
            ActionTriggerType::Attached => true,
            ActionTriggerType::CardDestroyed => true,
            ActionTriggerType::OtherCardDestroyed(_) => true,
            ActionTriggerType::PhaseStarted(phases, trigger_target) => match trigger_target {
                TriggerTarget::Owner => {
                    phases.contains(&turn.phase) && Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Opponent => {
                    phases.contains(&turn.phase) && !Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Any => phases.contains(&turn.phase) && true,
            },
            ActionTriggerType::CardPlayedFromHand(restrictions) => {
                if let Some((phases, trigger_target)) = restrictions {
                    let within_phase = phases.contains(&turn.phase);

                    match trigger_target {
                        TriggerTarget::Owner => {
                            within_phase && Arc::ptr_eq(&turn.current_player, &player)
                        }
                        TriggerTarget::Opponent => {
                            within_phase && !Arc::ptr_eq(&turn.current_player, &player)
                        }
                        TriggerTarget::Any => within_phase && true,
                    }
                } else {
                    true
                }
            }
            ActionTriggerType::Continuous => true,
            ActionTriggerType::Detached => true,
            // ActionTriggerType::CardTapped => false,
            // ActionTriggerType::CardTappedWithinPhases(phases) => phases.contains(&turn.phase),
            ActionTriggerType::AbilityWithinPhases(
                _,
                mana_requirements,
                phase_restrictions,
                _tap_required,
            ) => {
                let has_mana = player
                    .lock()
                    .await
                    .has_required_mana(mana_requirements)
                    .await;

                // let can_tap_card = not necesssary right meow
                let within_phase = if let Some((phases, trigger_target)) = phase_restrictions {
                    let within_phase = phases.contains(&turn.phase);

                    match trigger_target {
                        TriggerTarget::Owner => {
                            within_phase && Arc::ptr_eq(&turn.current_player, &player)
                        }
                        TriggerTarget::Opponent => {
                            within_phase && !Arc::ptr_eq(&turn.current_player, &player)
                        }
                        TriggerTarget::Any => within_phase && true,
                    }
                } else {
                    true
                };

                has_mana && within_phase
            }
            // ActionTriggerType::Sorcery => {
            //     turn.phase == TurnPhase::Main || turn.phase == TurnPhase::Main2
            // }
            ActionTriggerType::DamageApplied => turn.phase == TurnPhase::CombatDamage,
            ActionTriggerType::OtherCardPlayed(_trigger_target) => true,
            ActionTriggerType::CreatureTypeCardPlayed(_trigger_target, _creature_type) => true,
            ActionTriggerType::CardExiled => true,
            ActionTriggerType::OtherCardExiled(_trigger_target) => true,
            ActionTriggerType::HealthGained => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResetCardAction {}

#[async_trait::async_trait]
impl CardAction for ResetCardAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        _target: EffectTarget,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        game.effect_manager
            .remove_effects_by_source(&card, game.current_turn.clone().unwrap())
            .await;

        let card_id = {
            let card_lock = card.lock().await;
            card_lock.id.clone()
        };

        let fresh = card
            .lock()
            .await
            .owner
            .as_ref()
            .unwrap()
            .lock()
            .await
            .deck
            .fresh(card_id);

        if let Some(fresh_card) = fresh {
            let mut original_card_data = card.lock().await;

            *original_card_data = fresh_card;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlayCardAction {
    pub player_arc: Arc<Mutex<Player>>,
    pub card_arc: Arc<Mutex<Card>>,
    pub target: Option<EffectTarget>,
}

impl PlayCardAction {
    pub fn new(
        player_arc: Arc<Mutex<Player>>,
        card_arc: Arc<Mutex<Card>>,
        target: Option<EffectTarget>,
    ) -> Self {
        Self {
            player_arc,
            card_arc,
            target,
        }
    }
}

#[async_trait::async_trait]
impl Action for PlayCardAction {
    async fn apply(&self, game: &mut Game) -> Result<(), String> {
        println!("play card triggered.");
        if self.card_arc.lock().await.is_countered {
            // Move the card to the graveyard
            {
                let mut player = self.player_arc.lock().await;
                player.deck.destroy(self.card_arc.lock().await.id.clone());
            }
            game.add_turn_message(format!(
                "Spell {} was countered and moved to graveyard.",
                self.card_arc.lock().await.name
            ));
        } else {
            {
                let card = self.card_arc.lock().await;
                game.add_turn_message(format!(
                    "{} has played {}.",
                    card.owner.clone().unwrap().lock().await.name,
                    card.name
                ));
            }
            // Move the card to the battlefield
            {
                let mut player = self.player_arc.lock().await;
                player.cards_in_play.push(Arc::clone(&self.card_arc));
            }

            // Set the target and owner on the card
            {
                let mut card_lock = self.card_arc.lock().await;
                card_lock.target = self.target.clone();
                card_lock.action_target = self.target.clone();
                card_lock.owner = Some(self.player_arc.clone());
            }

            // Collect and execute any immediate actions
            // let mut actions = {
            //     let card_lock = self.card_arc.lock().await;
            //     card_lock.collect_phase_based_actions_sync(
            //         &game.current_turn.clone().unwrap(),
            //         ActionTriggerType::Instant,
            //         &self.card_arc,
            //     )
            // };
            // for action in actions {
            //     game.add_to_stack(action);
            // }
            // let _target = self.target.clone();
            // let mut actions = {
            //     let (actions, _) = self
            //         .card_arc
            //         .lock()
            //         .await
            //         .collect_manual_actions(
            //             Arc::clone(&self.card_arc),
            //             game.current_phase(),
            //             target.clone(),
            //         )
            //         .await;
            //     actions
            // };

            let mut actions = {
                let more_actions = game.collect_card_played_actions(&self.card_arc).await;
                more_actions
            };

            println!("instant actions? {:?}", actions);

            game.execute_actions(&mut actions).await?;

            // Handle special cases, e.g., if the card is a land
            {
                let card_lock = self.card_arc.lock().await;
                match card_lock.card_type {
                    CardType::AdvancedMultiLand(_, _) => {
                        let mut player = self.player_arc.lock().await;
                        player.mana_pool.played_card = true;
                    }
                    CardType::AdvancedLand(_) => {
                        let mut player = self.player_arc.lock().await;
                        player.mana_pool.played_card = true;
                    }
                    CardType::BasicLand(_) => {
                        let mut player = self.player_arc.lock().await;
                        player.mana_pool.played_card = true;
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CounterSpellAction {}

#[async_trait::async_trait]
impl CardAction for CounterSpellAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        _card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        if let EffectTarget::Card(target_action_arc) = &target {
            target_action_arc.lock().await.is_countered = true;
            game.debug("Countered a spell on the stack.");
        } else {
            println!("Target spell is not on the stack.");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReturnToHandAction {}

#[async_trait::async_trait]
impl CardAction for ReturnToHandAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        _game: &mut Game,
        _card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        if let EffectTarget::Card(target_card_arc) = target {
            let owner_arc = {
                let target_card = target_card_arc.lock().await;
                target_card.owner.clone()
            };

            if let Some(owner_arc) = owner_arc {
                let mut owner = owner_arc.lock().await;
                owner.return_card_to_hand(&target_card_arc).await;
                println!(
                    "Returned {} to {}'s hand.",
                    target_card_arc.lock().await.name,
                    owner.name
                );
            }
        } else {
            println!("No valid target for ReturnToHandAction.");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CardActionTrigger {
    pub id: String,
    pub trigger_type: ActionTriggerType,
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card_required_target: CardRequiredTarget,
    pub requirements: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                String,
            ) -> Pin<Box<dyn Future<Output = bool> + Send>>
            + Send
            + Sync,
    >,
}

impl Debug for CardActionTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardActionTrigger")
            .field("id", &self.id)
            .field("trigger_type", &self.trigger_type)
            .field("action", &self.action)
            .field("card_required_target", &self.card_required_target)
            .finish()
    }
}

impl CardActionTrigger {
    pub fn new_with_requirements(
        trigger_type: ActionTriggerType,
        card_required_target: CardRequiredTarget,
        action: Arc<dyn CardAction + Send + Sync>,
        requirements: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    Arc<Mutex<Card>>,
                    String,
                ) -> Pin<Box<dyn Future<Output = bool> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            trigger_type,
            card_required_target,
            action,
            requirements,
        }
    }

    pub fn new(
        trigger_type: ActionTriggerType,
        card_required_target: CardRequiredTarget,
        action: Arc<dyn CardAction + Send + Sync>,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            trigger_type,
            card_required_target,
            action,
            requirements: Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>,
                 ability_id: String|
                 -> Pin<Box<dyn Future<Output = bool> + Send>> {
                    Box::pin(async move { true })
                },
            ),
        }
    }
}

pub struct AsyncClosureWithCardAction {
    closure: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Card>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
            + Send
            + Sync,
    >,
}

impl AsyncClosureWithCardAction {
    pub fn new(
        closure: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    Arc<Mutex<Card>>,
                    Arc<Mutex<Card>>,
                ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { closure }
    }
}

#[async_trait::async_trait]
impl CardAction for AsyncClosureWithCardAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        source_card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        if let EffectTarget::Card(target_card) = target {
            let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

            let response = (self.closure)(game_arc.clone(), source_card, target_card).await;

            let mut game_unlocked = game_arc.lock().await;
            *game = std::mem::take(&mut *game_unlocked);

            return response;
        }
        Ok(())
    }
}
impl Debug for AsyncClosureWithCardAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncClosureWithCardAction").finish()
    }
}

pub struct AsyncClosureActionWithTargetAndAbility {
    closure: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                EffectTarget,
                String,
            ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
            + Send
            + Sync,
    >,
}

impl AsyncClosureActionWithTargetAndAbility {
    pub fn new(
        closure: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    Arc<Mutex<Card>>,
                    EffectTarget,
                    String,
                ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { closure }
    }
}

#[async_trait::async_trait]
impl CardAction for AsyncClosureActionWithTargetAndAbility {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        source_card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        if let Some(ability_id) = ability_id {
            let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

            let response = (self.closure)(game_arc.clone(), source_card, target, ability_id).await;

            let mut game_unlocked = game_arc.lock().await;
            *game = std::mem::take(&mut *game_unlocked);

            return response;
        }
        Ok(())
    }
}
impl Debug for AsyncClosureActionWithTargetAndAbility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncClosureActionWithAbilityId").finish()
    }
}

pub struct AsyncClosureAction {
    closure: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
            + Send
            + Sync,
    >,
}

impl AsyncClosureAction {
    pub fn new(
        closure: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    Arc<Mutex<Card>>,
                ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { closure }
    }
}

#[async_trait::async_trait]
impl CardAction for AsyncClosureAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        source_card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

        let response = (self.closure)(game_arc.clone(), source_card).await;

        let mut game_unlocked = game_arc.lock().await;
        *game = std::mem::take(&mut *game_unlocked);

        response
    }
}
impl Debug for AsyncClosureAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncClosureAction").finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTarget {
    Owner,
    Opponent,
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum CardRequiredTarget {
    None,
    OwnedCard,
    AnyPlayer,
    AnyCard,
    EnemyCard,
    EnemyPlayer,
    EnemyCardOrPlayer,
    CardOfType(CardType, CardTargetTeam, Option<bool>),
    BasicLand(CardTargetTeam, Option<bool>),
    CreatureOfType(CreatureType, CardTargetTeam, Option<bool>),
    EnemyCardInCombat,
    Spell,
    MultipleCardsOfType(CardType, i16),
    CreatureWithPowerAndToughness(i16, i16, CardTargetTeam),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum CardActionTarget {
    SelfCard,
    SelfOwner,
    CardTarget,
    EffectTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum CardTargetTeam {
    Owner,
    Opponent,
    Any,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum PlayerActionTarget {
    Owner,
    Opponent,
}

#[async_trait]
pub trait Action: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game) -> Result<(), String>;
}
#[derive(Debug, Clone)]
pub enum CardActionType {
    Manual,
    PhaseBased,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionTriggerType {
    CardPlayedFromHand(Option<(Vec<TurnPhase>, TriggerTarget)>),
    CardDestroyed,
    CardStatChanged,
    CardExiled,
    OtherCardExiled(TriggerTarget),
    // CardTapped,
    // CardTappedWithinPhases(Vec<TurnPhase>),
    // description, required mana, required phase(s), requires tap
    AbilityWithinPhases(
        String,
        Vec<ManaType>,
        Option<(Vec<TurnPhase>, TriggerTarget)>,
        bool,
    ),
    PhaseStarted(Vec<TurnPhase>, TriggerTarget),
    OtherCardPlayed(TriggerTarget),
    OtherCardDestroyed(TriggerTarget),
    CreatureTypeCardPlayed(TriggerTarget, CreatureType),
    DamageApplied,
    HealthGained,
    Attached,
    Detached,
    Continuous,
}

#[async_trait::async_trait]
pub trait CardAction: Send + Sync + Debug + 'static {
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String>;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone)]
pub struct CardActionWrapper {
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card: Arc<Mutex<Card>>,
    pub target: Option<EffectTarget>,
    pub ability_id: Option<String>,
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
    async fn apply(&self, game: &mut Game) -> Result<(), String> {
        let card = Arc::clone(&self.card);

        self.action
            .apply(
                game,
                card.clone(),
                self.target.clone().unwrap_or(EffectTarget::Card(card)),
                self.ability_id.clone(),
            )
            .await?;
        if let Some(ability_id) = &self.ability_id {
            let count = game
                .triggers_played_this_turn
                .get(ability_id.as_str())
                .unwrap_or(&0);
            println!("ability id {:?}", ability_id);
            if ability_id != "play_card" {
                game.cards_triggered_this_turn
                    .insert(self.card.lock().await.id.clone());
            }
            game.triggers_played_this_turn
                .insert(ability_id.clone(), count + 1);
        }
        Ok(())
    }
}

#[async_trait]
pub trait PlayerAction: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game, player_index: usize) -> Result<(), String>;
}

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
    async fn apply(&self, game: &mut Game) -> Result<(), String> {
        self.action.apply(game, self.player_index).await
    }
}

#[derive(Debug, Clone)]
pub struct ApplyStat {
    pub amount: i16,
    pub id: String,
    pub stat_type: StatType,
}

#[derive(Debug, Clone)]
pub struct ResetManaPoolAction {}

#[async_trait]
impl PlayerAction for ResetManaPoolAction {
    async fn apply(&self, game: &mut Game, player_index: usize) -> Result<(), String> {
        {
            let mut player = game.players[player_index].lock().await;
            println!("reseting mana pool.");

            player.mana_pool.empty_pool();
        }
        {
            game.effect_manager
                .apply_effects(game.current_turn.clone().unwrap())
                .await;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UntapAllAction {}

#[async_trait]
impl PlayerAction for UntapAllAction {
    async fn apply(&self, game: &mut Game, player_index: usize) -> Result<(), String> {
        let mut player = game.players[player_index].lock().await;

        for card in player.cards_in_play.iter() {
            card.lock().await.untap();
        }
        println!("untap all.");
        player.mana_pool.played_card = false;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DrawCardAction {
    pub target: PlayerActionTarget,
}

#[async_trait]
impl PlayerAction for DrawCardAction {
    async fn apply(&self, game: &mut Game, player_index: usize) -> Result<(), String> {
        match self.target {
            PlayerActionTarget::Owner => {
                game.players[player_index].lock().await.draw_card();
            }
            PlayerActionTarget::Opponent => {
                let opponent_index = (player_index + 1) % game.players.len();
                game.players[opponent_index].lock().await.draw_card();
            }
        }
        Ok(())
    }
}

#[async_trait]
pub trait Attachable: Debug + Send + Sync {
    async fn attach(
        self_arc: Arc<Mutex<Self>>,
        target: &Arc<Mutex<Card>>,
        game: &Game,
    ) -> Vec<Arc<dyn Action + Send + Sync>>;
    async fn detach(
        self_arc: Arc<Mutex<Self>>,
        target: &Arc<Mutex<Card>>,
        game: &Game,
    ) -> Vec<Arc<dyn Action + Send + Sync>>;
}

#[derive(Debug, Clone)]
pub struct CardDamageAction {
    // pub target: CardActionTarget,
}

#[async_trait]
impl CardAction for CardDamageAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let card = card.lock().await;
        // let target = &card.target;

        match target {
            EffectTarget::Player(target) => {
                let stats = &mut target.lock().await.stat_manager;
                let offense = card.get_stat_value(StatType::Power);
                let defense = stats.get_stat_value(StatType::Toughness);
                let total = offense - defense;
                println!("Do damage {} to {:?}", total, stats);
                stats
                    .add_stat(
                        Ulid::new().to_string(),
                        Stat::new(StatType::Health, -1 * total),
                    )
                    .await;
            }
            _ => todo!(),
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DestroyTargetCAction {}

#[async_trait]
impl CardAction for DestroyTargetCAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        match target {
            EffectTarget::Card(target_card) => {
                game.destroy_card(&target_card).await;
            }
            EffectTarget::Player(_) => {
                println!("Cannot destroy a player");
            }
            EffectTarget::CardId(_) => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DeclareBlockerAction {}

#[async_trait::async_trait]
impl CardAction for DeclareBlockerAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        match target {
            EffectTarget::Player(arc) => todo!(),
            EffectTarget::Card(arc) => {
                game.combat
                    .declare_blocker(Arc::clone(&card), Arc::clone(&arc))
                    .await?;
            }
            EffectTarget::CardId(_) => {}
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CombatAction {}

#[async_trait::async_trait]
impl PlayerAction for CombatAction {
    async fn apply(&self, game: &mut Game, player_index: usize) -> Result<(), String> {
        game.resolve_combat().await;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DeclareAttackerAction {}

#[async_trait::async_trait]
impl CardAction for DeclareAttackerAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        println!(
            "declaring \n\nattacker: {:?} \ntarget: {:?}\n\n",
            card, target
        );
        game.combat
            .declare_attacker(Arc::clone(&card), target)
            .await;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CombatDamageAction {
    pub attacking_creatures: Vec<Arc<Mutex<Card>>>,
    pub blocking_pairs: Vec<(Arc<Mutex<Card>>, Arc<Mutex<Card>>)>, // (Blocker, Attacker)
    pub defending_player: Arc<Mutex<Player>>,
}

#[async_trait]
pub trait DamageSource: Debug + Send + Sync {
    // Additional methods can be added as needed
}

#[async_trait]
pub trait DamageTarget: Debug + Send + Sync {
    async fn receive_damage(
        &mut self,
        amount: i16,
        source: &Arc<Mutex<dyn DamageSource + Send + Sync>>,
    );
}

#[derive(Clone, Debug)]
pub struct DamageAssignment {
    pub source: Arc<Mutex<dyn DamageSource + Send + Sync>>,
    pub target: Arc<Mutex<dyn DamageTarget + Send + Sync>>,
    pub damage: i16,
}

#[async_trait]
impl Action for DamageAssignment {
    async fn apply(&self, game: &mut Game) -> Result<(), String> {
        // Apply damage to the target
        self.target
            .lock()
            .await
            .receive_damage(self.damage, &self.source)
            .await;
        Ok(())
    }
}

fn is_attacker_blocked(
    attacker_arc: &Arc<Mutex<Card>>,
    blocked_attackers: &[Arc<Mutex<Card>>],
) -> bool {
    blocked_attackers
        .iter()
        .any(|blocked_attacker_arc| Arc::ptr_eq(attacker_arc, blocked_attacker_arc))
}

#[async_trait]
impl Action for CombatDamageAction {
    async fn apply(&self, game: &mut Game) -> Result<(), String> {
        // Step 1: Calculate damage assignments
        let mut damage_assignments = Vec::new();

        // Handle blocked attackers
        for (blocker_arc, attacker_arc) in &self.blocking_pairs {
            let attacker_damage = {
                let attacker = attacker_arc.lock().await;
                attacker.get_stat_value(StatType::Power)
            };

            let blocker_damage = {
                let blocker = blocker_arc.lock().await;
                blocker.get_stat_value(StatType::Power)
            };

            // Damage to blocker
            damage_assignments.push(DamageAssignment {
                source: attacker_arc.clone(),
                target: blocker_arc.clone(),
                damage: attacker_damage,
            });

            // Damage to attacker
            damage_assignments.push(DamageAssignment {
                source: blocker_arc.clone(),
                target: attacker_arc.clone(),
                damage: blocker_damage,
            });
        }

        // Handle unblocked attackers
        let blocked_attackers: Vec<Arc<Mutex<Card>>> = self
            .blocking_pairs
            .iter()
            .map(|(_, attacker_arc)| attacker_arc.clone())
            .collect();

        // In the CombatDamageAction implementation
        for attacker_arc in &self.attacking_creatures {
            if !is_attacker_blocked(attacker_arc, &blocked_attackers) {
                let attacker_damage = {
                    let attacker = attacker_arc.lock().await;
                    attacker.get_stat_value(StatType::Power)
                };

                // Damage to defending player
                damage_assignments.push(DamageAssignment {
                    source: attacker_arc.clone(),
                    target: self.defending_player.clone(),
                    damage: attacker_damage,
                });
            }
        }

        // Step 2: Deal damage
        for assignment in damage_assignments {
            assignment.apply(game).await?;
        }

        // Step 3: Handle deaths and destructions
        game.handle_deaths().await;
        Ok(())
    }
}

#[async_trait]
impl DamageSource for Card {
    // Implement necessary methods
}

#[async_trait]
impl DamageTarget for Card {
    async fn receive_damage(
        &mut self,
        amount: i16,
        source: &Arc<Mutex<dyn DamageSource + Send + Sync>>,
    ) {
        // Reduce card's defense or health
        self.modify_stat(StatType::Toughness, -amount).await;
        println!("{} takes {} damage.", self.name, amount);
    }
}

#[async_trait]
impl DamageSource for Player {
    // Implement necessary methods
}

#[async_trait]
impl DamageTarget for Player {
    async fn receive_damage(
        &mut self,
        amount: i16,
        source: &Arc<Mutex<dyn DamageSource + Send + Sync>>,
    ) {
        // Reduce player's health
        self.modify_stat(StatType::Health, -amount).await;
        println!("{} takes {} damage.", self.name, amount);
    }
}

pub struct ApplyDynamicEffectToCard {
    pub id: String,
    pub amount_calculator:
        Arc<dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>> + Send + Sync>,
    pub effects_generator: Arc<
        dyn Fn(
                EffectTarget,
                Arc<Mutex<Card>>,
                Arc<
                    dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>>
                        + Send
                        + Sync,
                >,
                String,
            )
                -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>>
            + Send
            + Sync,
    >,
}

impl ApplyDynamicEffectToCard {
    pub fn new(
        amount_calculator: Arc<
            dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>> + Send + Sync,
        >,
        effects_generator: Arc<
            dyn Fn(
                    EffectTarget,
                    Arc<Mutex<Card>>,
                    Arc<
                        dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>>
                            + Send
                            + Sync,
                    >,
                    String,
                ) -> Pin<
                    Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                > + Send
                + Sync,
        >,
    ) -> Self {
        ApplyDynamicEffectToCard {
            amount_calculator,
            effects_generator,
            id: Ulid::new().to_string(),
        }
    }
}

impl Debug for ApplyDynamicEffectToCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyDynamicEffectToCard").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyDynamicEffectToCard {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        // let amount_calculator = Arc::clone(&self.amount_calculator);
        // let amount = (&amount_calculator)(Arc::clone(&card_arc)).await;
        // if amount == 0 {
        // println!("hmmm, 0 amount?zc:");
        // }
        let effects = {
            let source_card = Arc::clone(&card_arc);

            // let owner = { &source_card.lock().await.owner.clone() };

            (self.effects_generator)(
                target,
                source_card,
                self.amount_calculator.clone(),
                self.id.clone(),
            )
            .await
        };

        for effect in effects {
            let effect_id = effect.lock().await.get_final_id();
            // println!("received effect from list {:?}", effect_id);

            game.effect_manager.add_effect(effect_id, effect);
        }
        Ok(())
    }
}

pub struct ApplyEffectsToPlayerCreatureType {
    pub id: String,
    // pub amount_calculator:
    //     Arc<dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>> + Send + Sync>,
    pub creature_type: CreatureType,
    pub effects_generator: Arc<
        dyn Fn(
                EffectTarget,
                Arc<Mutex<Card>>,
                // Arc<
                //     dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>>
                //         + Send
                //         + Sync,
                // >,
                String,
            )
                -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyEffectsToPlayerCreatureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToPlayerCreatureType").finish()
    }
}

impl ApplyEffectsToPlayerCreatureType {
    pub fn new(
        creature_type: CreatureType,
        effects_generator: Arc<
            dyn Fn(
                    EffectTarget,
                    Arc<Mutex<Card>>,
                    // Arc<
                    //     dyn Fn(Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = i16> + Send>>
                    //         + Send
                    //         + Sync,
                    // >,
                    String,
                ) -> Pin<
                    Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                > + Send
                + Sync,
        >,
    ) -> Self {
        ApplyEffectsToPlayerCreatureType {
            creature_type,
            effects_generator,
            id: Ulid::new().to_string(),
        }
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectsToPlayerCreatureType {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(owner_arc) = owner_arc {
            let owner = owner_arc.lock().await;

            // Iterate over the owner's in-play cards and apply the effect to matching card types
            for card_in_play in &owner.cards_in_play {
                let card_type_matches = {
                    let card = card_in_play.lock().await;
                    card.creature_type == Some(self.creature_type)
                };

                if card_type_matches {
                    let source_card = Arc::clone(&card_arc);
                    let effects = (self.effects_generator)(
                        EffectTarget::Card(Arc::clone(&card_in_play)),
                        source_card,
                        self.id.clone(),
                    )
                    .await;
                    for effect in effects {
                        // println!("received effect from list {:?}", effect_id);

                        let effect_id = effect.as_ref().lock().await.get_final_id().clone();
                        game.effect_manager.add_effect(effect_id, effect);
                    }
                }
            }
        } else {
            println!("No owner found for the card.");
        }
        Ok(())
    }
}

pub struct ApplyEffectToPlayerCardType {
    pub card_type: CardType,
    pub effect_generator: Arc<
        dyn Fn(EffectTarget, Option<Arc<Mutex<Card>>>) -> Vec<Arc<Mutex<dyn Effect + Send + Sync>>>
            + Send
            + Sync,
    >,
}

impl Debug for ApplyEffectToPlayerCardType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToPlayerCardType").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectToPlayerCardType {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(owner_arc) = owner_arc {
            let owner = owner_arc.lock().await;

            // Iterate over the owner's in-play cards and apply the effect to matching card types
            for card_in_play in &owner.cards_in_play {
                let card_type_matches = {
                    let card = card_in_play.lock().await;
                    card.card_type == self.card_type
                };

                if card_type_matches {
                    let source_card = Some(Arc::clone(&card_arc));
                    let effects = (self.effect_generator)(
                        EffectTarget::Card(Arc::clone(&card_in_play)),
                        source_card,
                    );

                    for effect in effects {
                        let effect_id = effect.lock().await.get_final_id();

                        game.effect_manager.add_effect(effect_id, effect);
                    }
                }
            }
        } else {
            println!("No owner found for the card.");
        }
        Ok(())
    }
}

pub struct ApplyEffectToTargetAction {
    pub effects_generator: Arc<
        dyn Fn(
                EffectTarget,
                Arc<Mutex<Card>>,
            )
                -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>>
            + Send
            + Sync,
    >,
}
impl ApplyEffectToTargetAction {
    pub fn new(
        effects_generator: Arc<
            dyn Fn(
                    EffectTarget,
                    Arc<Mutex<Card>>,
                ) -> Pin<
                    Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                > + Send
                + Sync,
        >,
    ) -> Self {
        ApplyEffectToTargetAction { effects_generator }
    }
}

impl Debug for ApplyEffectToTargetAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplyEffectToTargetAction").finish()
    }
}

#[async_trait::async_trait]
impl CardAction for ApplyEffectToTargetAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let source_card = Arc::clone(&card_arc);
        let effects = (self.effects_generator)(target.clone(), source_card).await;
        for effect in effects {
            let effect_id = effect.lock().await.get_final_id();
            println!("Adding effect {:?} to card {:?}", effect, target);

            game.effect_manager.add_effect(effect_id, effect);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct BlankAction {}

#[async_trait::async_trait]
impl CardAction for BlankAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        _game: &mut Game,
        _card_arc: Arc<Mutex<Card>>,
        _target: EffectTarget,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct TapCardAction {}

#[async_trait::async_trait]
impl CardAction for TapCardAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        println!("tapping card {:?} OR \n\n {:?}", card_arc, target);
        let mut card = card_arc.lock().await;
        card.tapped = true;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DrawCardCardAction {
    pub target: CardActionTarget,
    pub count: i16,
}

impl DrawCardCardAction {
    pub fn one(target: CardActionTarget) -> Self {
        DrawCardCardAction { target, count: 1 }
    }
}

#[async_trait::async_trait]
impl CardAction for DrawCardCardAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let card = card_arc.lock().await;
        let owner = card.owner.as_ref().unwrap();
        for _ in 0..self.count {
            owner.lock().await.draw_card();
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ChooseFromSelectionAction {
    pub details: CardSelectionDetails,
    pub action: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
}
impl Debug for ChooseFromSelectionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChooseFromSelectionAction").finish()
    }
}

impl ChooseFromSelectionAction {
    pub fn new(
        player_id: String,
        cards: Vec<CardWithDetails>,
        action: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
    ) -> Self {
        ChooseFromSelectionAction {
            details: CardSelectionDetails { player_id, cards },
            action,
        }
    }
}

#[async_trait]
impl CardAction for ChooseFromSelectionAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner = card_arc.lock().await.owner.clone();

        game.choose_action = Some(self.clone());

        game.ask_choose_from_selection(self.clone()).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct CastMandatoryAdditionalAbility {
    pub mana: Vec<ManaType>,
    pub target: CardRequiredTarget,
    pub ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
    pub description: String,
    pub action_type: ActionType,
}
impl Debug for CastMandatoryAdditionalAbility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CastMandatoryAdditionalAbility").finish()
    }
}

#[async_trait]
impl CardAction for CastMandatoryAdditionalAbility {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner = card_arc.lock().await.owner.clone();

        if let Some(owner_arc) = owner {
            let can_pay_mana_cost = { owner_arc.lock().await.can_pay_mana(&self.mana).await };
            if can_pay_mana_cost {
                let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

                Game::ask_mandatory_player_ability(
                    &game_arc,
                    Ability::new(
                        card_arc.clone(),
                        self.mana.clone(),
                        self.target.clone(),
                        self.ability.clone(),
                        None,
                        self.description.clone(),
                        self.action_type.clone(),
                    ),
                )
                .await;

                let mut game_unlocked = game_arc.lock().await;
                *game = std::mem::take(&mut *game_unlocked);
            } else {
                println!("Not enough mana to activate the ability.");
            }
        } else {
            println!("No owner found for the card.");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CastOptionalAdditionalAbility {
    pub mana: Vec<ManaType>,
    pub target: CardRequiredTarget,
    pub ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
    pub canceled: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
    pub description: String,
    pub action_type: ActionType,
}
impl Debug for CastOptionalAdditionalAbility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CastOptionalAdditionalAbility").finish()
    }
}

impl CastOptionalAdditionalAbility {
    pub fn new(
        mana: Vec<ManaType>,
        target: CardRequiredTarget,
        ability: Arc<dyn Fn(Arc<Mutex<Card>>) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
        description: String,
        action_type: ActionType,
    ) -> CastOptionalAdditionalAbility {
        let canceled = Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
            Arc::new(BlankAction {}) as Arc<dyn CardAction + Send + Sync>
        });
        CastOptionalAdditionalAbility {
            mana,
            target,
            ability,
            description,
            action_type,
            canceled,
        }
    }
}

#[async_trait]
impl CardAction for CastOptionalAdditionalAbility {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner = card_arc.lock().await.owner.clone();

        if let Some(owner_arc) = owner {
            println!("getting owner? {:?}", owner_arc);
            let can_pay_mana_cost = { owner_arc.lock().await.can_pay_mana(&self.mana).await };
            if can_pay_mana_cost {
                let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

                Game::request_player_ability(
                    &game_arc,
                    Ability::new(
                        card_arc.clone(),
                        self.mana.clone(),
                        self.target.clone(),
                        self.ability.clone(),
                        Some(self.canceled.clone()),
                        self.description.clone(),
                        self.action_type.clone(),
                    ),
                )
                .await;
                let mut game_unlocked = game_arc.lock().await;
                *game = std::mem::take(&mut *game_unlocked);
            } else {
                println!("Not enough mana to activate the ability.");
            }
        } else {
            println!("No owner found for the card.");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct LifeLinkAction {}

#[async_trait::async_trait]
impl CardAction for LifeLinkAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: &mut Game,
        card_arc: Arc<Mutex<Card>>,
        target: EffectTarget,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let (owner, amount, should) = {
            let lock = card_arc.lock().await;
            let owner = lock.owner.clone();
            let amount = lock.damage_dealt_to_players.clone();
            let should = lock.get_stat_value(StatType::Lifelink) > 0;
            (owner, amount, should)
        };

        if !should {
            return Ok(());
        }

        if let Some(owner) = owner {
            game.add_health(&owner, amount).await;
        }
        Ok(())
    }
}
