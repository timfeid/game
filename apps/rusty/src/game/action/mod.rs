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
    effects::{Effect, EffectID, ExpireContract},
    mana::{self, ManaType},
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

    pub async fn applies_in_phase(&self, turn: &Turn, player: Arc<Mutex<Player>>) -> bool {
        match &self.trigger_type {
            ActionTriggerType::CardStatChanged => true,
            ActionTriggerType::Attached => true,
            ActionTriggerType::CardDestroyed => true,
            ActionTriggerType::OtherCardDestroyed(_) => true,
            ActionTriggerType::PhaseStarted(phases, trigger_target) => match trigger_target {
                PhaseTarget::Owner => {
                    phases.contains(&turn.phase) && Arc::ptr_eq(&turn.current_player, &player)
                }
                PhaseTarget::Opponent => {
                    phases.contains(&turn.phase) && !Arc::ptr_eq(&turn.current_player, &player)
                }
                PhaseTarget::Any => phases.contains(&turn.phase) && true,
            },
            ActionTriggerType::CardEnteredBattlefield => false,
            ActionTriggerType::Omnipresent => true,
            ActionTriggerType::Detached => true,
            // ActionTriggerType::CardTapped => false,
            // ActionTriggerType::CardTappedWithinPhases(phases) => phases.contains(&turn.phase),
            ActionTriggerType::AbilityWithinPhases(
                _,
                mana_requirements,
                phase_restrictions,
                _tap_required,
                _target,
            ) => {
                let has_mana = player.lock().await.has_required_mana(mana_requirements);

                // let can_tap_card = not necesssary right meow
                let within_phase = if let Some((phases, phase_target)) = phase_restrictions {
                    let within_phase = phases.contains(&turn.phase);

                    match phase_target {
                        PhaseTarget::Owner => {
                            within_phase && Arc::ptr_eq(&turn.current_player, &player)
                        }
                        PhaseTarget::Opponent => {
                            within_phase && !Arc::ptr_eq(&turn.current_player, &player)
                        }
                        PhaseTarget::Any => within_phase && true,
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
            ActionTriggerType::PlayerStatChanged(_) => true,
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        _target: Option<FrontendTarget>,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        let turn = game.lock().await.current_turn.clone().unwrap();
        game.lock()
            .await
            .effect_manager
            .remove_effects_by_source(&card, turn)
            .await;

        let card_id = {
            let card_lock = card.lock().await;
            card_lock.id.clone()
        };

        let fresh = owner.lock().await.deck.fresh(card_id);

        if let Some(fresh_card) = fresh {
            let mut original_card_data = card.lock().await;

            *original_card_data = fresh_card;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlayCardAction {}

impl PlayCardAction {}

#[async_trait::async_trait]
impl CardAction for PlayCardAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        if let Ok(frontend_target) = Game::frontend_target_from_card(&game, &card).await {
            game.lock()
                .await
                .remove_from_frontend_target(&frontend_target)
                .await;
        }
        let countered = card.lock().await.is_countered;
        if countered {
            // Move the card to the graveyard
            {
                let mut player = player.lock().await;
                player.deck.destroy(card.lock().await.id.clone());
            }
            game.lock().await.add_turn_message(format!(
                "Spell {} was countered and moved to graveyard.",
                card.lock().await.name
            ));
        } else {
            println!("not countered");
            {
                let mut player = player.lock().await;
                player.cards_in_play.push(Arc::clone(&card));
            }

            let actions = {
                let more_actions = game
                    .lock()
                    .await
                    .collect_card_played_actions(&card, &target)
                    .await;
                more_actions
            };

            println!("instant actions? {:?}", actions);

            Game::execute_actions(game.clone(), actions).await?;

            // Handle special cases, e.g., if the card is a land
            {
                let card_lock = card.lock().await;
                match card_lock.card_type {
                    CardType::AdvancedMultiLand(_, _) => {
                        let mut player = player.lock().await;
                        player.mana_pool.played_card = true;
                    }
                    CardType::AdvancedLand(_) => {
                        let mut player = player.lock().await;
                        player.mana_pool.played_card = true;
                    }
                    CardType::BasicLand(_) => {
                        let mut player = player.lock().await;
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        if let Some(target) = &target {
            Game::card_from_frontend_target(&game, target)
                .await
                .lock()
                .await
                .is_countered = true;
            Ok(())
        } else {
            Err("No target?".to_string())
        }
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
        game: Arc<Mutex<Game>>,
        _card_arc: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        _ability_id: Option<String>,
    ) -> Result<(), String> {
        println!("\n\n\n{:?}\n\n{:?}\n\n\n", _card_arc, target);
        if let Some(FrontendTarget::Card(position)) = &target {
            let target_card_arc = Game::card_from_frontend_card_target(&game, position).await;

            {
                let mut owner = player.lock().await;
                owner.return_card_to_hand(&target_card_arc).await;
                game.lock().await.add_turn_message(format!(
                    "Returned {} to {}'s hand.",
                    target_card_arc.lock().await.name,
                    owner.name
                ));
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
                Arc<Mutex<Player>>,
                Option<String>,
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
    fn new(
        trigger_type: ActionTriggerType,
        card_required_target: CardRequiredTarget,
        action: Arc<dyn CardAction + Send + Sync>,
        requirements: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    Arc<Mutex<Card>>,
                    Arc<Mutex<Player>>,
                    Option<String>,
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
}

pub struct AsyncClosureAction {
    closure: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Player>>,
                Option<FrontendTarget>,
                String,
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
                    Arc<Mutex<Player>>,
                    Option<FrontendTarget>,
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
impl CardAction for AsyncClosureAction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        source_card: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        if let Some(ability_id) = ability_id {
            let response = (self.closure)(game, source_card, owner, target, ability_id).await;
            return response;
        }
        Ok(())
    }
}
impl Debug for AsyncClosureAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncClosureActionWithAbilityId").finish()
    }
}

#[derive(Debug, Serialize, Deserialize, Type, Clone, PartialEq)]
pub enum PhaseTarget {
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
    FrontendTarget,
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
    async fn apply(&self, game: Arc<Mutex<Game>>) -> Result<(), String>;
}
#[derive(Debug, Clone)]
pub enum CardActionType {
    Manual,
    PhaseBased,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionTriggerType {
    CardEnteredBattlefield,
    CardDestroyed,
    CardStatChanged,
    CardExiled,
    OtherCardExiled(PhaseTarget),
    // CardTapped,
    // CardTappedWithinPhases(Vec<TurnPhase>),
    // description, required mana, required phase(s), requires tap
    AbilityWithinPhases(
        String,
        Vec<ManaType>,
        Option<(Vec<TurnPhase>, PhaseTarget)>,
        bool,
        CardRequiredTarget,
    ),
    PhaseStarted(Vec<TurnPhase>, PhaseTarget),
    OtherCardPlayed(PhaseTarget),
    OtherCardDestroyed(PhaseTarget),
    CreatureTypeCardPlayed(PhaseTarget, CreatureType),
    DamageApplied,
    PlayerStatChanged(StatType),
    Attached,
    Detached,
    Omnipresent,
}

#[async_trait::async_trait]
pub trait CardAction: Send + Sync + Debug + 'static {
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String>;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone)]
pub struct CardActionWrapper {
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card: Arc<Mutex<Card>>,
    pub target: Option<FrontendTarget>,
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
    async fn apply(&self, game: Arc<Mutex<Game>>) -> Result<(), String> {
        let card = Arc::clone(&self.card);
        let owner = card.lock().await.owner.clone().unwrap();

        self.action
            .apply(
                game.clone(),
                card.clone(),
                owner,
                self.target.clone(),
                self.ability_id.clone(),
            )
            .await?;

        if let Some(ability_id) = &self.ability_id {
            let count = game
                .lock()
                .await
                .triggers_played_this_turn
                .get(ability_id.as_str())
                .unwrap_or(&0)
                .clone();

            game.lock()
                .await
                .triggers_played_this_turn
                .insert(ability_id.clone(), count + 1);
        }
        Ok(())
    }
}

#[async_trait]
pub trait PlayerAction: Send + Sync + Debug {
    async fn apply(&self, game: Arc<Mutex<Game>>, player: Arc<Mutex<Player>>)
        -> Result<(), String>;
}

#[derive(Clone)]
pub struct PlayerActionWrapper {
    pub action: Arc<dyn PlayerAction + Send + Sync>,
    pub player: Arc<Mutex<Player>>,
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
    async fn apply(&self, game: Arc<Mutex<Game>>) -> Result<(), String> {
        self.action.apply(game, self.player.clone()).await
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
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
    ) -> Result<(), String> {
        player.lock().await.mana_pool.empty_pool();

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UntapAllAction {}

#[async_trait]
impl PlayerAction for UntapAllAction {
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
    ) -> Result<(), String> {
        let cards_in_play = player.lock().await.cards_in_play.clone();

        for card in cards_in_play.iter() {
            card.lock().await.untap();
        }
        player.lock().await.mana_pool.played_card = false;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DrawCardAction {}

#[async_trait]
impl PlayerAction for DrawCardAction {
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
    ) -> Result<(), String> {
        player.lock().await.draw_card();
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let card = card.lock().await;
        // let target = &card.target;

        if let Some(target) = &target {
            match target {
                FrontendTarget::Player(player_id) => {
                    let (_, target) = Game::player_from_frontend_target(&game, target).await?;

                    let stats = &mut target.lock().await.stat_manager;
                    let offense = card.get_stat_value(StatType::Power);
                    let defense = stats.get_stat_value(StatType::Toughness);
                    let total = offense - defense;
                    println!("Do damage {} to {:?}", total, stats);
                    stats.add_stat(
                        Ulid::new().to_string(),
                        Stat::new(StatType::Health, -1 * total),
                    );
                }
                _ => todo!(),
            };
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DestroySelf {}

#[async_trait]
impl CardAction for DestroySelf {
    fn as_any(&self) -> &dyn Any {
        self
    }
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        Game::destroy_card(&game, &card).await
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        match &target {
            None => Err(format!("Nothing to destroy")),
            Some(FrontendTarget::Card(frontend_target)) => {
                let target_card =
                    Game::card_from_frontend_card_target(&game, frontend_target).await;
                Game::destroy_card(&game, &target_card).await
            }
            Some(FrontendTarget::Player(_)) => Err(format!("Cannot destroy a player")),
        }
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        match &target {
            None => todo!(),
            Some(FrontendTarget::Player(arc)) => todo!(),
            Some(FrontendTarget::Card(frontend_target)) => {
                let arc = Game::card_from_frontend_card_target(&game, frontend_target).await;
                game.lock()
                    .await
                    .combat
                    .declare_blocker(Arc::clone(&card), Arc::clone(&arc))
                    .await?;
            }
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CombatAction {}

#[async_trait::async_trait]
impl PlayerAction for CombatAction {
    async fn apply(
        &self,
        game: Arc<Mutex<Game>>,
        player: Arc<Mutex<Player>>,
    ) -> Result<(), String> {
        Game::resolve_combat(&game).await;
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
        game: Arc<Mutex<Game>>,
        card: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        println!(
            "declaring \n\nattacker: {:?} \ntarget: {:?}\n\n",
            card, target
        );
        if let Some(target) = target {
            game.lock()
                .await
                .combat
                .declare_attacker(Arc::clone(&card), target)
                .await;
        }
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
    async fn apply(&self, game: Arc<Mutex<Game>>) -> Result<(), String> {
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
    async fn apply(&self, game: Arc<Mutex<Game>>) -> Result<(), String> {
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
            assignment.apply(game.clone()).await?;
        }

        // Step 3: Handle deaths and destructions
        game.lock().await.handle_deaths().await;
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
        self.modify_stat(StatType::Toughness, -amount);
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
        self.modify_stat(StatType::Health, -amount);
        println!("{} takes {} damage.", self.name, amount);
    }
}

pub struct ApplyDynamicEffectToCard {
    pub id: String,
    pub effects_generator: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                FrontendTarget,   // target
                Arc<Mutex<Card>>, //source
                String,           // effect id
            )
                -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>>
            + Send
            + Sync,
    >,
}

impl ApplyDynamicEffectToCard {
    pub fn new(
        effects_generator: Arc<
            dyn Fn(
                    Arc<Mutex<Game>>,
                    FrontendTarget,
                    Arc<Mutex<Card>>,
                    String,
                ) -> Pin<
                    Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                > + Send
                + Sync,
        >,
    ) -> Self {
        ApplyDynamicEffectToCard {
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        // let amount_calculator = Arc::clone(&self.amount_calculator);
        // let amount = (&amount_calculator)(Arc::clone(&card_arc)).await;
        // if amount == 0 {
        // println!("hmmm, 0 amount?zc:");
        // }
        if let Some(target) = target {
            let effects = {
                let source_card = Arc::clone(&card_arc);

                // let owner = { &source_card.owner.clone() };

                (self.effects_generator)(game.clone(), target, source_card, self.id.clone()).await
            };

            for effect in effects {
                let effect_id = effect.lock().await.get_final_id();
                // println!("received effect from list {:?}", effect_id);

                game.lock()
                    .await
                    .effect_manager
                    .add_effect(effect_id, effect);
            }
        }
        Ok(())
    }
}

pub struct ApplyEffectsToPlayerCreatureType {
    pub id: String,
    pub creature_type: CreatureType,
    pub effects_generator: Arc<
        dyn Fn(
                Arc<Mutex<Card>>, // source card
                Arc<Mutex<Card>>, // creature card
                String,           // trigger id
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
                    Arc<Mutex<Card>>, // source card
                    Arc<Mutex<Card>>, // creature card
                    String,           // trigger id
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        _: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let owner_arc = {
            let card = card_arc.lock().await;
            card.owner.clone()
        };

        if let Some(owner_arc) = owner_arc {
            let owner = owner_arc.lock().await;

            // Iterate over the owner's in-play cards and apply the effect to matching card types
            for card_in_play in owner.cards_in_play.clone() {
                let card_type_matches = {
                    let card = card_in_play.lock().await;
                    card.creature_type == Some(self.creature_type)
                };

                if card_type_matches {
                    let source_card = Arc::clone(&card_arc);
                    let effects =
                        (self.effects_generator)(source_card, card_in_play, self.id.clone()).await;
                    for effect in effects {
                        // println!("received effect from list {:?}", effect_id);

                        let effect_id = effect.lock().await.get_final_id().clone();
                        game.lock()
                            .await
                            .effect_manager
                            .add_effect(effect_id, effect);
                    }
                }
            }
        } else {
            println!("No owner found for the card.");
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
        _game: Arc<Mutex<Game>>,
        _card_arc: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        _target: Option<FrontendTarget>,
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        player: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let card = card_arc.lock().await;
        for _ in 0..self.count {
            owner.lock().await.draw_card();
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ChooseFromSelectionAction {
    pub details: CardSelectionDetails,
    pub action: Arc<dyn Fn(FrontendCardTarget) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
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
        action: Arc<dyn Fn(FrontendCardTarget) -> Arc<dyn CardAction + Send + Sync> + Send + Sync>,
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        game.lock().await.choose_action = Some(self.clone());

        game.lock()
            .await
            .ask_choose_from_selection(self.clone())
            .await;
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let can_pay_mana_cost = { owner.lock().await.can_pay_mana(&self.mana).await };
        if can_pay_mana_cost {
            let mut game = game.lock().await;
            game.ask_mandatory_player_ability(Ability::new(
                card_arc.clone(),
                owner.clone(),
                self.mana.clone(),
                self.target.clone(),
                self.ability.clone(),
                None,
                self.description.clone(),
                self.action_type.clone(),
            ))
            .await;
        } else {
            println!("Not enough mana to activate the ability.");
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
            Arc::new(PlayCardAction {}) as Arc<dyn CardAction + Send + Sync>
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let can_pay_mana_cost = { owner.lock().await.can_pay_mana(&self.mana).await };
        if can_pay_mana_cost {
            game.lock()
                .await
                .request_player_ability(Ability::new(
                    card_arc.clone(),
                    owner.clone(),
                    self.mana.clone(),
                    self.target.clone(),
                    self.ability.clone(),
                    Some(self.canceled.clone()),
                    self.description.clone(),
                    self.action_type.clone(),
                ))
                .await;
        } else {
            println!("Not enough mana to activate the ability.");
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
        game: Arc<Mutex<Game>>,
        card_arc: Arc<Mutex<Card>>,
        owner: Arc<Mutex<Player>>,
        target: Option<FrontendTarget>,
        ability_id: Option<String>,
    ) -> Result<(), String> {
        let (amount, should) = {
            let lock = card_arc.lock().await;
            let amount = lock.damage_dealt_to_players.clone();
            let should = lock.get_stat_value(StatType::Lifelink) > 0;
            (amount, should)
        };

        if !should {
            return Ok(());
        }

        Game::add_stat(&game, &card_arc, &owner, StatType::Health, amount).await;
        Ok(())
    }
}

pub struct ActionBuilder {
    trigger_type: ActionTriggerType,
    target: CardRequiredTarget,
    action: Option<Arc<dyn CardAction + Send + Sync>>,
    requirements: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Player>>,
                Option<String>,
            ) -> Pin<Box<dyn Future<Output = bool> + Send>>
            + Send
            + Sync,
    >,
}

impl ActionBuilder {
    pub fn new(trigger_type: ActionTriggerType) -> Self {
        Self {
            trigger_type,
            target: CardRequiredTarget::None,
            action: None,
            requirements: Arc::new(|_, _, _, _| Box::pin(async move { true })),
        }
    }

    pub fn closure_action<F>(mut self, action: F) -> Self
    where
        F: Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Player>>,
                Option<FrontendTarget>,
                String,
            ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
            + 'static
            + Send
            + Sync,
    {
        self.action = Some(Arc::new(AsyncClosureAction::new(Arc::new(action))));
        self
    }

    pub fn action<F>(mut self, action: F) -> Self
    where
        F: CardAction + Send + Sync,
    {
        self.action = Some(Arc::new(action));
        self
    }

    pub fn target(mut self, target: CardRequiredTarget) -> Self {
        self.target = target;
        self
    }

    pub fn requirements<F>(mut self, requirements: F) -> Self
    where
        F: Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Player>>,
                Option<String>,
            ) -> Pin<Box<dyn Future<Output = bool> + Send>>
            + 'static
            + Send
            + Sync,
    {
        self.requirements = Arc::new(requirements);
        self
    }

    pub fn build(self) -> CardActionTrigger {
        CardActionTrigger::new(
            self.trigger_type,
            self.target,
            self.action.unwrap(),
            self.requirements,
        )
    }
}
