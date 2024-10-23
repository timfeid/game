use crate::{
    game::{
        action::{
            generate_mana::GenerateManaAction, Action, ActionTriggerType, ApplyDynamicEffectToCard,
            ApplyEffectToPlayerCardType, ApplyEffectToTargetAction,
            ApplyEffectsToPlayerCreatureType, AsyncClosureAction, AsyncClosureAction,
            AsyncClosureWithCardAction, BlankAction, CardAction, CardActionTarget,
            CardActionTrigger, CardActionWrapper, CardRequiredTarget, CardTargetTeam,
            CastMandatoryAdditionalAbility, CastOptionalAdditionalAbility,
            ChooseFromSelectionAction, DamageTarget, DeclareAttackerAction, DeclareBlockerAction,
            DrawCardAction, DrawCardCardAction, PhaseTarget, PlayCardAction, PlayerActionTarget,
            ReturnToHandAction, TapCardAction,
        },
        card::{
            card::{create_creature_card, create_multiple_cards},
            Card, CardPhase, CardType, Counter, CreatureType,
        },
        decks::duplicate_card,
        effects::{
            DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract,
            ModifyStatTarget, StatModifierEffect,
        },
        mana::ManaType,
        player::Player,
        stat::{Stat, StatType, StaticStatId, Stats},
        turn::TurnPhase,
        ActionType, CardWithDetails, Game,
    },
    lobby::manager::{CardSelectionDetails, LobbyCommand},
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

fn create_mana_elf_trigger() -> CardActionTrigger {
    CardActionTrigger::new(
        ActionTriggerType::AbilityWithinPhases(
            "Add {G} for each Elf on the battlefield.".to_string(),
            vec![],
            None,
            true,
        ),
        CardRequiredTarget::None,
        Arc::new(AsyncClosureAction::new(Arc::new(
            |game: Arc<Mutex<Game>>,
             card: Arc<Mutex<Card>>|
             -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                Box::pin(async move {
                    let owner = card.lock().await.owner.clone().unwrap();
                    let cards_in_play = &owner.lock().await.cards_in_play.clone();
                    for card in cards_in_play {
                        if card.lock().await.creature_type == Some(CreatureType::Elf) {
                            owner.lock().await.mana_pool.add_mana(ManaType::Green);
                        }
                    }
                    Ok(())
                })
            },
        ))),
    )
}

pub fn create_tyvar_kell() -> Card {
    Card::new(
        "Tyvar Kell",
        "Elves you control have \"{T}: Add {B}.\"",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![TurnPhase::Main, TurnPhase::Main2],
                    PhaseTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                            let owner = source.lock().await.owner.clone();
                            if let Some(owner) = owner {
                                let cards = owner.lock().await.cards_in_play.clone();
                                for card in cards {
                                    let creature_type = card.lock().await.creature_type.clone();
                                    if creature_type == Some(CreatureType::Elf) {
                                        card.lock().await.triggers.push(
                                            create_mana_elf_trigger()
                                        );
                                    }
                                }
                            }

                            Ok(())
                        })
                    }
                ))),
            ),
            CardActionTrigger::new(
                ActionTriggerType::OtherCardPlayed(
                    PhaseTarget::Owner,
                ),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>, target| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                            let creature_type = target.lock().await.creature_type.clone();
                            if creature_type == Some(CreatureType::Elf) {

                                target.lock().await.triggers.push(
                                    create_mana_elf_trigger()
                                );
                            }

                            Ok(())
                        })
                    }
                ))),
            ),
            CardActionTrigger::new_with_requirements(
                ActionTriggerType::AbilityWithinPhases(
                    "[+1]: Put a +1/+1 counter on up to one target Elf. Untap it. It gains deathtouch until end of turn.".to_string(),
                    vec![],
                    Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
                    false,
                ),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, source_card: Arc<Mutex<Card>>, target, ability_id| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                            let ability_id = ability_id.clone();
                            source_card.lock().await.current_phase = CardPhase::Charging(1);
                            Card::add_counter(source_card.clone(), &game, Counter::Incremental(1)).await;
                            game.lock().await.execute_actions(
                                &mut vec![
                                    Arc::new(CardActionWrapper {
                                        ability_id: Some(ability_id.clone()),
                                        card: source_card.clone(),
                                        action: Arc::new(CastOptionalAdditionalAbility::new(
                                            vec![],
                                            CardRequiredTarget::CreatureOfType(
                                                CreatureType::Elf,
                                                CardTargetTeam::Any,
                                                None
                                            ),
                                            Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(
                                                    Arc::new(|game, source_card, target| {
                                                        Box::pin(async move {
                                                            Card::add_counter(target.clone(), &game, Counter::PowerToughnessModifier(1, 1)).await;
                                                            target.lock().await.untap();
                                                            game.lock().await.effect_manager.add_effect(EffectID("asdf".to_string()),
                                                                Arc::new(
                                                                    Mutex::new(
                                                                        StatModifierEffect::new(
                                                                            EffectTarget::Card(target.clone()),
                                                                            StatType::Deathtouch,
                                                                            1,
                                                                            ExpireContract::Turns(1),
                                                                            Some(source_card.clone()),
                                                                        )
                                                                    )
                                                                ) as Arc<Mutex<dyn Effect + Send + Sync>>
                                                            );
                                                            Ok(())
                                                        })
                                                    }),
                                                )) as Arc<dyn CardAction + Send + Sync>
                                            }),
                                            "Put a +1/+1 counter on up to one target Elf. Untap it. It gains deathtouch until end of turn.".to_string(),
                                            ActionType::None,
                                        )),
                                        target: None
                                    })
                                ]
                            ).await?;
                            Ok(())
                    })
                }))),

                Arc::new(
                    |game: Arc<Mutex<Game>>,
                     card: Arc<Mutex<Card>>,
                     ability_id|
                     -> Pin<Box<dyn Future<Output = bool> + Send>> {
                        Box::pin(async move {
                            card.lock().await.current_phase == CardPhase::Ready
                        })
                    }
                )
            ),
            CardActionTrigger::new_with_requirements(
                ActionTriggerType::AbilityWithinPhases(
                    "[0]: Create a 1/1 green Elf Warrior creature token.".to_string(),
                    vec![],
                    Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
                    false,
                ),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, source_card: Arc<Mutex<Card>>, target, ability_id| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                            let owner = source_card.lock().await.owner.clone();
                            if let Some(owner) = owner {
                                Game::play_token(&game, &owner, create_creature_card!("Token - Elf Warrior", CreatureType::Elf, "", 1,1, [], [])).await.ok();
                            }
                            Ok(())
                    })
                }))),

                Arc::new(
                    |game: Arc<Mutex<Game>>,
                     card: Arc<Mutex<Card>>,
                     ability_id|
                     -> Pin<Box<dyn Future<Output = bool> + Send>> {
                        Box::pin(async move {
                            card.lock().await.current_phase == CardPhase::Ready
                        })
                    }
                )
            ),
            CardActionTrigger::new_with_requirements(
                ActionTriggerType::AbilityWithinPhases(
                    "[−6]: You get an emblem with \"Whenever you cast an Elf spell, it gains haste until end of turn and you draw two cards.\"".to_string(),
                    vec![],
                    Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
                    false,
                ),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                            if source.lock().await.get_stat_value(StatType::Counter) < 6 {
                                return Err("Cannot go below 0".to_string());
                            }

                            source.lock().await.current_phase = CardPhase::Charging(1);
                            Card::add_counter(source.clone(), &game, Counter::Incremental(-6)).await;
                            let owner = source.lock().await.owner.clone();
                            if let Some(owner) = owner {
                                source.lock().await.triggers.push(
                                    CardActionTrigger::new(
                                        ActionTriggerType::OtherCardPlayed(PhaseTarget::Owner),
                                        CardRequiredTarget::None,
                                        Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                            |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>, card_played: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                Box::pin(async move {

                                                    let (owner, creature_type, toughness) = {
                                                        let card = card_played.lock().await;
                                                        let owner = card.owner.clone().unwrap();
                                                        let creature_type = card.creature_type.clone();
                                                        let toughness = card.get_stat_value(StatType::Toughness);
                                                        (owner, creature_type, toughness)
                                                    };

                                                    if creature_type == Some(CreatureType::Elf) {
                                                        owner.lock().await.draw_card();
                                                        owner.lock().await.draw_card();
                                                        game.lock().await.effect_manager.add_effect(EffectID("haist".to_string()),
                                                            Arc::new(
                                                                Mutex::new(
                                                                    StatModifierEffect::new(
                                                                        EffectTarget::Card(card_played.clone()),
                                                                        StatType::Haist,
                                                                        1,
                                                                        ExpireContract::Turns(1),
                                                                        Some(source.clone()),
                                                                    )
                                                                )
                                                            ) as Arc<Mutex<dyn Effect + Send + Sync>>
                                                        );

                                                    }
                                                    Ok(())
                                                })
                                            }
                                        )))
                                    )
                            );
                            }

                            Ok(())
                        })
                    }
                ))),
                Arc::new(
                    |game: Arc<Mutex<Game>>,
                     card: Arc<Mutex<Card>>,
                     ability_id|
                     -> Pin<Box<dyn Future<Output = bool> + Send>> {
                        Box::pin(async move {
                            let card = card.lock().await;
                            card.get_stat_value(StatType::Counter) > 5 && card.current_phase == CardPhase::Ready
                        })
                    }
                )
            )
        ],
        CardPhase::Ready,
        CardType::Planeswalker,
        vec![Stat::new(StatType::Counter, 3)],
        vec![
            ManaType::Colorless,
            ManaType::Colorless,
            ManaType::Green,
            ManaType::Green,
        ],
    )
}
