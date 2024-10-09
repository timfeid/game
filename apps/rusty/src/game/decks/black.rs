use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionTriggerType, ApplyDynamicEffectToCard,
        ApplyEffectToCardBasedOnTotalCardType, AsyncClosureAction, AsyncClosureWithCardAction,
        CardActionTrigger, CardRequiredTarget, CardTargetTeam, DeclareAttackerAction,
        DeclareBlockerAction, PlayerActionTarget, TriggerTarget,
    },
    card::{
        card::{create_creature_card, create_multiple_cards},
        Card, CardPhase, CardType, CreatureType,
    },
    decks::duplicate_card,
    effects::{
        DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract, LifeLinkAction,
    },
    mana::ManaType,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    Game,
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

fn create_swamp() -> Card {
    Card::new(
        "Swamp",
        "",
        vec![CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add 1 {B} to your pool".to_string(),
                vec![],
                None,
                true,
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Black],
                target: PlayerActionTarget::Owner,
            }),
        )],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Black),
        vec![],
        vec![],
    )
}

pub fn create_vengful_spirit() -> Card {
    create_creature_card!(
        "Vengeful Spirit",
        CreatureType::None,
        "When Vengeful Spirit deals combat damage to a player, you gain that much life.",
        3, // Damage
        2, // Defense
        [ManaType::Black],
        [StatType::Lifelink],
        CardActionTrigger::new(
            ActionTriggerType::PhaseStarted(vec![TurnPhase::CombatDamage], TriggerTarget::Owner),
            CardRequiredTarget::None,
            Arc::new(LifeLinkAction {})
        )
    )
}

pub fn create_hydra() -> Card {
    create_creature_card!(
        "Voracious Hydra",
        CreatureType::None,
        "Hydra that grows with X mana",
        0,
        1,
        [ManaType::Black],
        [],
        CardActionTrigger::new(
            ActionTriggerType::Continuous,
            CardRequiredTarget::None,
            Arc::new(ApplyDynamicEffectToCard::new(
                Arc::new(
                    move |card_arc: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = i8> + Send>> {
                        Box::pin(async move {
                            let owner = {
                                let card = card_arc.lock().await;
                                card.owner.clone()
                            };

                            if let Some(owner_arc) = owner {
                                let owner = owner_arc.lock().await;
                                owner.mana_pool.total() as i8
                            } else {
                                0
                            }
                        })
                    },
                ),
                Arc::new(
                    move |target,
                          source_card,
                          amount,
                          effect_id|
                          -> Pin<
                        Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                    > {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];
                            let mut effect = DynamicStatModifierEffect::new(
                                target.clone(),
                                StatType::Power,
                                amount.clone(),
                                ExpireContract::Never,
                                Some(source_card.clone()),
                                false,
                            );

                            let id = format!(
                                "{}-{}-power",
                                source_card.clone().lock().await.id,
                                effect_id
                            );
                            effect.id = EffectID(id);
                            effects.push(Arc::new(Mutex::new(effect)));
                            let mut effect = DynamicStatModifierEffect::new(
                                target,
                                StatType::Toughness,
                                amount,
                                ExpireContract::Never,
                                Some(source_card.clone()),
                                false,
                            );
                            let id = format!(
                                "{}-{}-toughness",
                                source_card.clone().lock().await.id,
                                effect_id
                            );
                            effect.id = EffectID(id);
                            effects.push(Arc::new(Mutex::new(effect)));

                            effects
                        })
                    },
                ),
            )),
        )
    )
}

pub fn create_blanchwood_armor() -> Card {
    Card::new(
        "Blanchwood Armor",
        "Enchanted creature gets +1/+1 for each Forest you control",
        vec![CardActionTrigger::new(
            ActionTriggerType::Attached,
            CardRequiredTarget::CardOfType(CardType::Creature, CardTargetTeam::Any),
            Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                card_type: CardType::BasicLand(ManaType::Black),
                effects_generator: Arc::new(|target, source_card, amount_calculator| {
                    vec![
                        Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                            target.clone(),
                            StatType::Power,
                            amount_calculator.clone(),
                            ExpireContract::Never,
                            source_card.clone(),
                            false,
                        ))),
                        Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                            target,
                            StatType::Toughness,
                            amount_calculator.clone(),
                            ExpireContract::Never,
                            source_card.clone(),
                            false,
                        ))),
                    ]
                }),
            }),
        )],
        CardPhase::Ready,
        CardType::Enchantment,
        vec![],
        vec![ManaType::Black],
    )
}

pub fn create_black_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_blanchwood_armor(), 4));
    deck.append(&mut duplicate_card(create_swamp(), 8));
    deck.append(&mut duplicate_card(create_hydra(), 4));

    deck
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::game::{
        card::Card,
        decks::{
            black::{create_hydra, create_swamp},
            Deck,
        },
        effects::EffectTarget,
        mana,
        player::Player,
        Game,
    };

    #[tokio::test]
    async fn test_black_1() {
        // DeclareAttackerAction
        // TurnPhase
        // DeclareBlockerAction
        // Stat
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                vec![
                    create_swamp(),
                    create_swamp(),
                    create_swamp(),
                    create_hydra(),
                ],
            ))
            .await;

        player.lock().await.draw_card();
        player.lock().await.draw_card();
        player.lock().await.draw_card();
        let hydra = player.lock().await.draw_card();
        game.start_turn(0).await;

        {
            let clone = Arc::clone(&player);
            let mut player = clone.lock().await;
            let mut cards: Vec<Arc<Mutex<Card>>> = player.cards_in_hand.drain(0..3).collect();

            // Append the drained cards to `cards_in_play`
            player.cards_in_play.append(&mut cards);
        }

        let ga = Arc::new(Mutex::new(game));
        Game::process_action_queue(ga.clone(), hydra.clone().unwrap()).await;

        ga.lock()
            .await
            .activate_card_action_old(&player, 1, None)
            .await
            .expect("oh no?");

        ga.lock().await.print().await;

        // ga.lock().await.advance_turn().await;
    }
}
