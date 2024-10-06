pub mod add_stat;
pub mod generate_mana;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
    future::Future,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::Mutex;
use ulid::Ulid;

use super::{
    card::{Card, CardType, CreatureType},
    effects::{Effect, EffectID, EffectTarget, ExpireContract},
    mana::ManaType,
    player::Player,
    stat::{StatType, Stats},
    turn::{Turn, TurnPhase},
    FrontendCardTarget, FrontendTarget, Game,
};
use crate::{game::stat::Stat, lobby::manager::LobbyCommand};

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
        if let ActionTriggerType::CardTapped = self.trigger_type {
            self.action.apply(game, index).await;
        }
    }

    pub(crate) async fn applies_in_phase(&self, turn: Turn, player: Arc<Mutex<Player>>) -> bool {
        match &self.trigger_type {
            ActionTriggerType::CardTapped => false,
            ActionTriggerType::Attached => true,
            ActionTriggerType::CardDestroyed => true,
            ActionTriggerType::PhaseStarted(phases, trigger_target) => match trigger_target {
                TriggerTarget::Owner => {
                    phases.contains(&turn.phase) && Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Target => {
                    phases.contains(&turn.phase) && !Arc::ptr_eq(&turn.current_player, &player)
                }
                TriggerTarget::Any => phases.contains(&turn.phase) && true,
            },
            ActionTriggerType::CardInPlay => true,
            ActionTriggerType::Detached => true,
            ActionTriggerType::CardTappedWithinPhases(phases) => phases.contains(&turn.phase),
            ActionTriggerType::ActionedWithinPhases(mana_requirements, phases) => {
                player
                    .lock()
                    .await
                    .has_required_mana(mana_requirements)
                    .await
                    && phases.contains(&turn.phase)
            }
            // ActionTriggerType::Sorcery => {
            //     turn.phase == TurnPhase::Main || turn.phase == TurnPhase::Main2
            // }
            ActionTriggerType::DamageApplied => turn.phase == TurnPhase::CombatDamage,
            ActionTriggerType::OtherCardPlayed(trigger_target) => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResetCardAction {}

#[async_trait::async_trait]
impl CardAction for ResetCardAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        {
            let card = card.clone();
            let mut card_l = card.lock().await;
            card_l.is_countered = false;
        }

        game.effect_manager
            .remove_effects_by_source(&card, game.current_turn.clone().unwrap())
            .await;
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

#[derive(Debug)]
pub struct CounterSpellAction {}

#[async_trait::async_trait]
impl CardAction for CounterSpellAction {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
        if let EffectTarget::Card(target_action_arc) = &target {
            target_action_arc.lock().await.is_countered = true;
            game.debug("Countered a spell on the stack.");
        } else {
            println!("Target spell is not on the stack.");
        }
    }
}

#[async_trait::async_trait]
impl Action for PlayCardAction {
    async fn apply(&self, game: &mut Game) {
        println!("play card triggered.");
        if self.card_arc.lock().await.is_countered {
            // Move the card to the graveyard
            {
                let mut player = self.player_arc.lock().await;
                player.deck.destroy(Arc::clone(&self.card_arc));
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
            let target = self.target.clone();
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

            game.execute_actions(&mut actions).await;

            // Handle special cases, e.g., if the card is a land
            {
                let card_lock = self.card_arc.lock().await;
                if let CardType::BasicLand(_) = card_lock.card_type {
                    let mut player = self.player_arc.lock().await;
                    player.mana_pool.played_card = true;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReturnToHandAction {}

#[async_trait::async_trait]
impl CardAction for ReturnToHandAction {
    async fn apply(&self, game: &mut Game, card_arc: Arc<Mutex<Card>>, target: EffectTarget) {
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
    }
}

#[derive(Debug, Clone)]
pub struct CardActionTrigger {
    pub trigger_type: ActionTriggerType,
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card_required_target: CardRequiredTarget,
}

impl CardActionTrigger {
    pub fn new(
        trigger_type: ActionTriggerType,
        card_required_target: CardRequiredTarget,
        action: Arc<dyn CardAction + Send + Sync>,
    ) -> Self {
        Self {
            trigger_type,
            card_required_target,
            action,
        }
    }

    pub async fn trigger(&self, game: &mut Game, card: Arc<Mutex<Card>>) {
        let effect_target = match &self.trigger_type {
            ActionTriggerType::PhaseStarted(vec, trigger_target) => {
                let owner = {
                    if trigger_target == &TriggerTarget::Owner {
                        &card.lock().await.owner.clone()
                    } else {
                        &None
                    }
                };

                if let Some(owner) = owner {
                    EffectTarget::Player(Arc::clone(owner))
                } else {
                    EffectTarget::Card(Arc::clone(&card))
                }
            }
            // ActionTriggerType::Tap => todo!(),
            // ActionTriggerType::OnDamageApplied => todo!(),
            // ActionTriggerType::Instant => todo!(),
            // ActionTriggerType::Attached => todo!(),
            // ActionTriggerType::Detached => todo!(),
            // ActionTriggerType::OnCardDestroyed => todo!(),
            // ActionTriggerType::Sorcery => todo!(),
            _ => {
                if let Some(target) = card.lock().await.action_target.clone() {
                    target
                } else if let Some(target) = card.lock().await.attached.clone() {
                    EffectTarget::Card(target)
                } else if let Some(target) = card.lock().await.target.clone() {
                    target
                } else {
                    EffectTarget::Card(Arc::clone(&card))
                }
            }
        };

        match &self.trigger_type {
            ActionTriggerType::CardTapped => {
                self.action.apply(game, card, effect_target).await;
            }
            ActionTriggerType::Attached => {
                self.action.apply(game, card, effect_target).await;
            }
            ActionTriggerType::CardInPlay => {
                self.action.apply(game, card, effect_target).await;
            }
            x => {
                println!("nothing triggered for {:?}", x);
            }
        }
    }
}

pub struct AsyncClosureWithCardAction {
    closure: Arc<
        dyn Fn(
                Arc<Mutex<Game>>,
                Arc<Mutex<Card>>,
                Arc<Mutex<Card>>,
            ) -> Pin<Box<dyn Future<Output = ()> + Send>>
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
                ) -> Pin<Box<dyn Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { closure }
    }
}

#[async_trait::async_trait]
impl CardAction for AsyncClosureWithCardAction {
    async fn apply(&self, game: &mut Game, source_card: Arc<Mutex<Card>>, target: EffectTarget) {
        if let EffectTarget::Card(target_card) = target {
            let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

            (self.closure)(game_arc.clone(), source_card, target_card).await;

            let mut game_unlocked = game_arc.lock().await;
            *game = std::mem::take(&mut *game_unlocked);
        }
    }
}
impl Debug for AsyncClosureWithCardAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncClosureWithCardAction").finish()
    }
}

pub struct AsyncClosureAction {
    closure: Arc<
        dyn Fn(Arc<Mutex<Game>>, Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = ()> + Send>>
            + Send
            + Sync,
    >,
}

impl AsyncClosureAction {
    pub fn new(
        closure: Arc<
            dyn Fn(Arc<Mutex<Game>>, Arc<Mutex<Card>>) -> Pin<Box<dyn Future<Output = ()> + Send>>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { closure }
    }
}

#[async_trait::async_trait]
impl CardAction for AsyncClosureAction {
    async fn apply(&self, game: &mut Game, source_card: Arc<Mutex<Card>>, target: EffectTarget) {
        let game_arc = Arc::new(Mutex::new(std::mem::take(game)));

        (self.closure)(game_arc.clone(), source_card).await;

        let mut game_unlocked = game_arc.lock().await;
        *game = std::mem::take(&mut *game_unlocked);
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
    Target,
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
    CardOfType(CardType),
    CreatureOfType(CreatureType),
    EnemyCardInCombat,
    Spell,
    MultipleCardsOfType(CardType, i8),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub enum CardActionTarget {
    SelfCard,
    SelfOwner,
    CardTarget,
    EffectTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerActionTarget {
    SelfPlayer,
    Opponent,
}

#[async_trait]
pub trait Action: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game);
}
#[derive(Debug, Clone)]
pub enum CardActionType {
    Manual,
    PhaseBased,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionTriggerType {
    CardInPlay,
    CardDestroyed,
    CardTapped,
    CardTappedWithinPhases(Vec<TurnPhase>),
    ActionedWithinPhases(Vec<ManaType>, Vec<TurnPhase>),
    PhaseStarted(Vec<TurnPhase>, TriggerTarget),
    OtherCardPlayed(TriggerTarget),
    DamageApplied,
    Attached,
    Detached,
}

#[async_trait::async_trait]
pub trait CardAction: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget);
}

#[derive(Clone)]
pub struct CardActionWrapper {
    pub action: Arc<dyn CardAction + Send + Sync>,
    pub card: Arc<Mutex<Card>>,
    pub target: Option<EffectTarget>,
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

        self.action
            .apply(
                game,
                card.clone(),
                self.target.clone().unwrap_or(EffectTarget::Card(card)),
            )
            .await;
    }
}

#[async_trait]
pub trait PlayerAction: Send + Sync + Debug {
    async fn apply(&self, game: &mut Game, player_index: usize);
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
    async fn apply(&self, game: &mut Game) {
        self.action.apply(game, self.player_index).await;
    }
}

#[derive(Debug, Clone)]
pub struct ApplyStat {
    pub amount: i8,
    pub id: String,
    pub stat_type: StatType,
}

#[derive(Debug, Clone)]
pub struct ResetManaPoolAction {}

#[async_trait]
impl PlayerAction for ResetManaPoolAction {
    async fn apply(&self, game: &mut Game, player_index: usize) {
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
    }
}

#[derive(Debug, Clone)]
pub struct UntapAllAction {}

#[async_trait]
impl PlayerAction for UntapAllAction {
    async fn apply(&self, game: &mut Game, player_index: usize) {
        let mut player = game.players[player_index].lock().await;

        for card in player.cards_in_play.iter() {
            card.lock().await.untap();
        }
        println!("untap all.");
        player.mana_pool.played_card = false;
    }
}

#[derive(Debug, Clone)]
pub struct DrawCardAction {
    pub target: PlayerActionTarget,
}

#[async_trait]
impl PlayerAction for DrawCardAction {
    async fn apply(&self, game: &mut Game, player_index: usize) {
        match self.target {
            PlayerActionTarget::SelfPlayer => {
                game.players[player_index].lock().await.draw_card();
            }
            PlayerActionTarget::Opponent => {
                let opponent_index = (player_index + 1) % game.players.len();
                game.players[opponent_index].lock().await.draw_card();
            }
        }
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
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        let card = card.lock().await;
        // let target = &card.target;

        match target {
            EffectTarget::Player(target) => {
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct DestroyTargetCAction {}

#[async_trait]
impl CardAction for DestroyTargetCAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        match target {
            EffectTarget::Card(target_card) => {
                game.destroy_card(&target_card).await;
            }
            EffectTarget::Player(_) => {
                println!("Cannot destroy a player");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeclareBlockerAction {}

#[async_trait::async_trait]
impl CardAction for DeclareBlockerAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        match target {
            EffectTarget::Player(arc) => todo!(),
            EffectTarget::Card(arc) => {
                game.combat
                    .declare_blocker(Arc::clone(&card), Arc::clone(&arc))
                    .await;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CombatAction {}

#[async_trait::async_trait]
impl PlayerAction for CombatAction {
    async fn apply(&self, game: &mut Game, player_index: usize) {
        let destroyed_cards = game.combat.resolve_combat().await;
        for card in destroyed_cards {
            game.destroy_card(&card).await;
        }
        game.handle_deaths().await;
    }
}

#[derive(Debug, Clone)]
pub struct DeclareAttackerAction {}

#[async_trait::async_trait]
impl CardAction for DeclareAttackerAction {
    async fn apply(&self, game: &mut Game, card: Arc<Mutex<Card>>, target: EffectTarget) {
        println!(
            "declaring \n\nattacker: {:?} \ntarget: {:?}\n\n",
            card, target
        );
        game.combat
            .declare_attacker(Arc::clone(&card), target)
            .await;
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
        amount: i8,
        source: &Arc<Mutex<dyn DamageSource + Send + Sync>>,
    );
}

#[derive(Clone, Debug)]
pub struct DamageAssignment {
    pub source: Arc<Mutex<dyn DamageSource + Send + Sync>>,
    pub target: Arc<Mutex<dyn DamageTarget + Send + Sync>>,
    pub damage: i8,
}

#[async_trait]
impl Action for DamageAssignment {
    async fn apply(&self, game: &mut Game) {
        // Apply damage to the target
        self.target
            .lock()
            .await
            .receive_damage(self.damage, &self.source)
            .await;
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
    async fn apply(&self, game: &mut Game) {
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
            assignment.apply(game).await;
        }

        // Step 3: Handle deaths and destructions
        game.handle_deaths().await;
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
        amount: i8,
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
        amount: i8,
        source: &Arc<Mutex<dyn DamageSource + Send + Sync>>,
    ) {
        // Reduce player's health
        self.modify_stat(StatType::Health, -amount);
        println!("{} takes {} damage.", self.name, amount);
    }
}
